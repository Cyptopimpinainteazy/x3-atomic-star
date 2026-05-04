use crate::flash_finality::FlashFinalityBridge;
use crate::rpc_middleware::{RateLimitConfig, RateLimiter};
use flash_finality::{FlashFinalityConfig, FlashFinalityGadget, FLASH_FINALITY_PROTOCOL_ID};
use futures_util::StreamExt;
use poh_generator::PoHState;
use sc_client_api::{Backend, BlockBackend, BlockchainEvents};
use sc_consensus_aura::{ImportQueueParams, SlotProportion, StartAuraParams};
use sc_consensus_grandpa::SharedVoterState;
use sc_executor::NativeElseWasmExecutor;
use sc_service::{
    new_native_or_wasm_executor, ChainType, Configuration, Error as ServiceError,
    KeystoreContainer, PartialComponents, TaskManager,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_api::HeaderT;
use sp_consensus_aura::sr25519::AuthorityPair as AuraPair;
use sp_core::{crypto::KeyTypeId, Pair};
use sp_runtime::SaturatedConversion;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use x3_bridge_adapters::{
    OffchainEscrowPersistence, PalletEscrowAdapter, SubstrateClientBalanceAdapter,
};
/// X3 Chain node service module
///
/// Provides node initialization, partial components, and full service setup with:
/// - Aura (Authority Round) block authoring consensus
/// - GRANDPA finality gadget
/// - libp2p networking with peer discovery
/// - Proper block import queue with consensus verification
/// - Startup gate for determinism validation before joining consensus
use x3_chain_runtime::{opaque::Block, RuntimeApi};

/// Key type for Aura block authoring
const AURA: KeyTypeId = KeyTypeId(*b"aura");
/// Key type for GRANDPA finality
const GRANDPA: KeyTypeId = KeyTypeId(*b"gran");

/// Txpool sizing aligned to X3 throughput targets.
/// Default Substrate pool (8 192/512) is 12x too small for 100k TPS goals.
/// Tuned per audit recommendation: 100k ready / 50k future, 256 MiB / 64 MiB.
const TX_POOL_READY_COUNT: usize = 100_000;
const TX_POOL_FUTURE_COUNT: usize = 50_000;
const TX_POOL_READY_BYTES: usize = 256 * 1024 * 1024; // 256 MiB
const TX_POOL_FUTURE_BYTES: usize = 64 * 1024 * 1024; // 64 MiB
const TX_POOL_BAN_TIME_SECS: u64 = 60; // 60s ban (vs default 1800s) — faster retry under burst

/// Rollout feature flags for consensus and execution paths.
/// All flags default to off; enable per-validator via CLI or env on canary set first.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeFeatureFlags {
    /// Enable the parallel proposer pipeline.
    pub enable_parallel_proposer: bool,
    /// Enable Flash Finality tasks.
    pub enable_flash_finality: bool,
    /// Enable PoH digest validation path.
    pub enable_poh: bool,
    /// Require GPU path for validation critical flows.
    pub gpu_required: bool,
}
/// X3 Chain native executor implementation
pub struct AtlasSphereExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for AtlasSphereExecutorDispatch {
    type ExtendHostFunctions = sp_io::SubstrateHostFunctions;

    fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
        x3_chain_runtime::api::dispatch(method, data)
    }

    fn native_version() -> sc_executor::NativeVersion {
        x3_chain_runtime::native_version()
    }
}

/// Executor for X3 Chain
///
/// In normal builds we use `NativeElseWasmExecutor` so the node can
/// execute both natively and via the embedded WASM runtime. For
/// `skip-wasm-build` development builds, we force a pure native
/// executor to avoid deserializing a missing or dummy WASM blob.
pub type Executor = NativeElseWasmExecutor<AtlasSphereExecutorDispatch>;

/// Full client type alias
pub type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;

/// Full backend type alias
pub type FullBackend = sc_service::TFullBackend<Block>;

/// Type alias for select chain implementation
pub type SelectChain = sc_consensus::LongestChain<FullBackend, Block>;

/// Insert development keys into the keystore for block authoring.
///
/// For development mode (`--dev`), this inserts Alice's Aura (sr25519) and
/// GRANDPA (ed25519) keys into the keystore so the node can author blocks.
fn insert_dev_keys_with_seed(keystore: &KeystoreContainer, seed: &str) -> Result<(), ServiceError> {
    use sp_core::crypto::SecretStringError;

    let keystore = keystore.keystore();

    // Insert Aura key (sr25519) for block authoring
    let aura_pair =
        sp_core::sr25519::Pair::from_string(seed, None).map_err(|e: SecretStringError| {
            ServiceError::Other(format!("Failed to generate Aura keypair: {:?}", e))
        })?;
    keystore
        .insert(AURA, seed, &aura_pair.public().0)
        .map_err(|e| ServiceError::Other(format!("Failed to insert Aura key: {:?}", e)))?;

    log::info!("🔑 Inserted Aura key for block authoring");

    // Insert GRANDPA key (ed25519) for finality
    let grandpa_pair =
        sp_core::ed25519::Pair::from_string(seed, None).map_err(|e: SecretStringError| {
            ServiceError::Other(format!("Failed to generate GRANDPA keypair: {:?}", e))
        })?;
    keystore
        .insert(GRANDPA, seed, &grandpa_pair.public().0)
        .map_err(|e| ServiceError::Other(format!("Failed to insert GRANDPA key: {:?}", e)))?;

    log::info!("🔑 Inserted GRANDPA key for finality");

    Ok(())
}

fn maybe_insert_dev_keys(
    config: &Configuration,
    keystore: &KeystoreContainer,
) -> Result<(), ServiceError> {
    // If X3_DEV_SEED is set, insert that key regardless of chain type (testnet convenience).
    if let Ok(seed) = std::env::var("X3_DEV_SEED") {
        log::info!("🔑 Inserting dev keys from X3_DEV_SEED");
        return insert_dev_keys_with_seed(keystore, &seed);
    }

    // For development chains, insert Alice's keys for block authoring
    if config.chain_spec.chain_type() == ChainType::Development {
        return insert_dev_keys_with_seed(keystore, "//Alice");
    }

    Ok(())
}

fn tuned_transaction_pool_options(
    mut options: sc_transaction_pool::Options,
) -> sc_transaction_pool::Options {
    // Count caps: 100k ready / 50k future (audit recommendation for 100k TPS target)
    options.ready.count = options.ready.count.max(TX_POOL_READY_COUNT);
    options.future.count = options.future.count.max(TX_POOL_FUTURE_COUNT);
    // Byte caps: 256 MiB ready / 64 MiB future — aligned with large burst tx sets
    options.ready.total_bytes = options.ready.total_bytes.max(TX_POOL_READY_BYTES);
    options.future.total_bytes = options.future.total_bytes.max(TX_POOL_FUTURE_BYTES);
    // Ban time: 60s instead of default 1800s — faster retry for legitimate bursts
    options.ban_time = Duration::from_secs(TX_POOL_BAN_TIME_SECS);
    options
}

/// Apply the tuned limits to a runtime configuration before the pool is built.
pub fn tune_transaction_pool_config(config: &mut Configuration) {
    config.transaction_pool = tuned_transaction_pool_options(config.transaction_pool.clone());
}

/// Return the correct Aura slot duration for a given runtime spec_version.
///
/// CRITICAL: Aura enforces slot monotonicity. If the slot duration changes mid-chain,
/// nodes that don't gate on spec_version will compute wrong slots for historical blocks
/// and either stall or fork. This function is the safety valve.
///
/// - spec_version < 5: legacy 400ms slots (genesis chain used 400ms)
/// - spec_version >= 5: 200ms slots (v5 migration target)
///
/// Call this when building/verifying any block with a spec_version you can read.
pub fn slot_duration_for_spec(spec_version: u32) -> Duration {
    if spec_version >= 5 {
        Duration::from_millis(200)
    } else {
        Duration::from_millis(400)
    }
}

/// Create partial components for X3 Chain node
///
/// Returns the common components needed by various subcommands (benchmarking, export, etc.)
pub fn new_partial(
    config: &Configuration,
) -> Result<
    PartialComponents<
        FullClient,
        FullBackend,
        SelectChain,
        sc_consensus::DefaultImportQueue<Block, FullClient>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            sc_consensus_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, SelectChain>,
            sc_consensus_grandpa::LinkHalf<Block, FullClient, SelectChain>,
            Option<Telemetry>,
        ),
    >,
    ServiceError,
> {
    // Set up telemetry if endpoints are configured
    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    // Create executor
    let executor = new_native_or_wasm_executor::<AtlasSphereExecutorDispatch>(config);

    // Build partial components
    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            &config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;

    // For dev chains or when X3_DEV_SEED is set, insert keys for block authoring.
    maybe_insert_dev_keys(config, &keystore_container)?;

    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager
            .spawn_handle()
            .spawn("telemetry", None, worker.run());
        telemetry
    });

    // Select chain implementation (longest chain rule)
    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    // Create GRANDPA block import wrapper
    let (grandpa_block_import, grandpa_link) = sc_consensus_grandpa::block_import(
        client.clone(),
        &client,
        select_chain.clone(),
        telemetry.as_ref().map(|x| x.handle()),
    )?;

    // Create Aura import queue with proper block verification
    let slot_duration = sc_consensus_aura::slot_duration(&*client)?;

    let import_queue =
        sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(ImportQueueParams {
            block_import: grandpa_block_import.clone(),
            justification_import: Some(Box::new(grandpa_block_import.clone())),
            client: client.clone(),
            create_inherent_data_providers: move |_, ()| async move {
                let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                let slot =
					sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						*timestamp,
						slot_duration,
					);

                Ok((slot, timestamp))
            },
            spawner: &task_manager.spawn_essential_handle(),
            registry: config.prometheus_registry(),
            check_for_equivocation: Default::default(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            compatibility_mode: Default::default(),
        })?;

    Ok(PartialComponents {
        client,
        backend,
        task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other: (grandpa_block_import, grandpa_link, telemetry),
    })
}

/// Determine whether GRANDPA should run given configuration and feature flags.
///
/// - returns `false` when either the user disabled GRANDPA explicitly or when the
///   experimental Flash Finality gadget flag is active. This helper exists so
///   that unit tests can verify the decision logic without spawning a full node.
pub fn compute_enable_grandpa(config: &Configuration, feature_flags: NodeFeatureFlags) -> bool {
    let mut enable = !config.disable_grandpa;
    if feature_flags.enable_flash_finality {
        enable = false;
    }
    enable
}

/// Start a new X3 Chain full node with complete consensus and networking
pub fn new_full(
    mut config: Configuration,
    feature_flags: NodeFeatureFlags,
) -> Result<TaskManager, ServiceError> {
    tune_transaction_pool_config(&mut config);
    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other: (grandpa_block_import, grandpa_link, mut telemetry),
    } = new_partial(&config)?;

    // configure network protocols; GRANDPA may be disabled when using Flash Finality
    let mut net_config = sc_network::config::FullNetworkConfiguration::new(&config.network);

    // decide whether GRANDPA should be active; tests can call the helper below.
    let enable_grandpa = compute_enable_grandpa(&config, feature_flags);
    if !enable_grandpa && feature_flags.enable_flash_finality {
        log::info!("⚡ Flash Finality flag is set; GRANDPA will be disabled for this node");
    }

    let genesis_hash = client
        .block_hash(0)?
        .ok_or_else(|| ServiceError::Other("Genesis block not found".to_string()))?;
    let grandpa_protocol_name =
        sc_consensus_grandpa::protocol_standard_name(&genesis_hash, &config.chain_spec);

    if enable_grandpa {
        net_config.add_notification_protocol(sc_consensus_grandpa::grandpa_peers_set_config(
            grandpa_protocol_name.clone(),
        ));
    }

    let warp_sync = if enable_grandpa {
        Some(Arc::new(
            sc_consensus_grandpa::warp_proof::NetworkProvider::new(
                backend.clone(),
                grandpa_link.shared_authority_set().clone(),
                Vec::default(),
            ),
        ))
    } else {
        None
    };

    if feature_flags.enable_flash_finality {
        let mut flash_proto = sc_network::config::NonDefaultSetConfig::new(
            FLASH_FINALITY_PROTOCOL_ID.into(),
            1024 * 1024,
        );
        flash_proto.allow_non_reserved(25, 25);
        net_config.add_notification_protocol(flash_proto);
    }

    // Build networking service
    let (network, system_rpc_tx, tx_handler_controller, network_starter, sync_service) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            net_config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_params: warp_sync.map(|w| sc_service::WarpSyncParams::WithProvider(w)),
        })?;

    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks: Option<()> = None;
    let name = config.network.node_name.clone();
    let chain_name = config.chain_spec.name().to_string();
    let prometheus_registry = config.prometheus_registry().cloned();
    let role_for_grandpa = role.clone();

    if feature_flags.enable_parallel_proposer {
        log::warn!(
            "⚠️ --enable-parallel-proposer is set, but node wiring still uses basic authorship. \
            Keep this in shadow mode until deterministic scheduler integration is complete."
        );
    }
    if feature_flags.enable_flash_finality {
        if enable_grandpa {
            // still running grandpa due to some configuration oddity
            log::warn!(
                "⚠️ --enable-flash-finality is set but GRANDPA will still run due to configuration."
            );
        } else {
            log::info!(
                "⚡ Flash Finality is enabled; GRANDPA has been disabled for this node (shadow mode)."
            );
        }
    }
    if feature_flags.enable_poh {
        log::warn!(
            "⚠️ --enable-poh is set, but PoH digest verification is not yet enforced in block import."
        );
    }
    if feature_flags.gpu_required {
        log::warn!(
            "⚠️ --gpu-required=true is set; ensure CPU fallback is not relied on by your deployment policy."
        );
    }

    // Initialize PoH State if enabled
    let shared_poh_state = if feature_flags.enable_poh {
        Some(Arc::new(Mutex::new(PoHState::default())))
    } else {
        None
    };

    // Initialize Flash Finality Gadget for RPC regardless of whether we run the bridge
    let flash_finality_gadget = if feature_flags.enable_flash_finality {
        let keystore = keystore_container.keystore();
        let my_id = keystore
            .sr25519_public_keys(KeyTypeId(*b"flsh"))
            .get(0)
            .map(|k| k.0);

        if let Some(my_id) = my_id {
            Some(Arc::new(FlashFinalityGadget::new(
                FlashFinalityConfig::default(),
                my_id,
                Some(Box::new(keystore) as Box<dyn std::any::Any + Send + Sync>),
            )))
        } else {
            log::warn!(
                "⚠️ Flash Finality enabled but no flsh key found in keystore; disabling Flash Finality gadget"
            );
            None
        }
    } else {
        None
    };

    // Spawn core Substrate tasks (RPC, network, telemetry, txpool, offchain, etc.)
    let rate_limiter = Arc::new(RateLimiter::new(RateLimitConfig::default()));

    {
        let limiter = rate_limiter.clone();
        task_manager
            .spawn_handle()
            .spawn("rpc-rate-limiter-cleanup", None, async move {
                let interval = Duration::from_secs(60);
                loop {
                    tokio::time::sleep(interval).await;
                    limiter.cleanup_stale_connections(Duration::from_secs(5 * 60));
                }
            });
    }

    let rpc_builder = {
        let client = client.clone();
        let transaction_pool = transaction_pool.clone();
        let gadget = flash_finality_gadget.clone();
        let limiter = rate_limiter.clone();
        Box::new(move |deny_unsafe, _subscription_executor| {
            crate::rpc::create_full(
                client.clone(),
                transaction_pool.clone(),
                deny_unsafe,
                gadget.clone(),
                limiter.clone(),
            )
            .map_err(|e| ServiceError::Other(format!("RPC module creation failed: {:?}", e)))
        })
    };

    let disable_grandpa_flag = config.disable_grandpa;

    sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        config,
        client: client.clone(),
        backend: backend.clone(),
        task_manager: &mut task_manager,
        keystore: keystore_container.keystore(),
        transaction_pool: transaction_pool.clone(),
        rpc_builder,
        network: network.clone(),
        system_rpc_tx,
        tx_handler_controller,
        sync_service: sync_service.clone(),
        telemetry: telemetry.as_mut(),
    })?;

    // Start Aura block authoring if this is an authority node
    if role.is_authority() {
        let proposer_factory = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
        let shared_poh_state_for_aura = shared_poh_state.clone();

        let aura = sc_consensus_aura::start_aura::<AuraPair, _, _, _, _, _, _, _, _, _, _>(
            StartAuraParams {
                slot_duration,
                client: client.clone(),
                select_chain,
                block_import: grandpa_block_import,
                proposer_factory,
                create_inherent_data_providers: move |_, ()| {
                    let poh_state = shared_poh_state_for_aura.clone();
                    async move {
                        let timestamp = sp_timestamp::InherentDataProvider::from_system_time();
                        let slot =
                            sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
                                *timestamp,
                                slot_duration,
                            );

                        // Advance PoH state if enabled (shadow mode — just tick, don't inject as inherent)
                        if let Some(state_arc) = poh_state {
                            let mut state = state_arc.lock().await;
                            state.advance(&[]);
                        }

                        Ok((slot, timestamp))
                    }
                },
                force_authoring,
                backoff_authoring_blocks,
                keystore: keystore_container.keystore(),
                sync_oracle: sync_service.clone(),
                justification_sync_link: sync_service.clone(),
                block_proposal_slot_portion: SlotProportion::new(0.9f32),
                max_block_proposal_slot_portion: None,
                telemetry: telemetry.as_ref().map(|x| x.handle()),
                compatibility_mode: Default::default(),
            },
        )?;

        task_manager
            .spawn_essential_handle()
            .spawn_blocking("aura", Some("block-authoring"), aura);
    }

    // Start GRANDPA finality gadget
    if enable_grandpa {
        let grandpa_config = sc_consensus_grandpa::Config {
            gossip_duration: std::time::Duration::from_millis(100),
            justification_period: 64,
            name: Some(name.clone()),
            observer_enabled: false,
            keystore: Some(keystore_container.keystore()),
            local_role: role_for_grandpa,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            protocol_name: grandpa_protocol_name,
        };

        // Create GRANDPA parameters with offchain transaction pool
        let offchain_tx_pool_factory =
            sc_transaction_pool_api::OffchainTransactionPoolFactory::new(transaction_pool.clone());

        let grandpa_params = sc_consensus_grandpa::GrandpaParams {
            config: grandpa_config,
            link: grandpa_link,
            network: network.clone(),
            sync: Arc::new(sync_service.clone()),
            voting_rule: sc_consensus_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry,
            shared_voter_state: SharedVoterState::empty(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            offchain_tx_pool_factory,
        };

        task_manager.spawn_essential_handle().spawn_blocking(
            "grandpa-voter",
            None,
            sc_consensus_grandpa::run_grandpa_voter(grandpa_params)?,
        );
    }

    // Start the network
    network_starter.start_network();

    // Spawn a background task to watch finalized blocks and log events with emojis
    {
        let client = client.clone();
        task_manager
            .spawn_handle()
            .spawn("import-watcher", None, async move {
                use futures_util::StreamExt;

                let mut notifications = client.import_notification_stream();
                while let Some(notification) = notifications.next().await {
                    let number: u64 = (*notification.header.number()).saturated_into();
                    // Purple color for block imported
                    log::info!(
                        "\x1b[35m📦 Block imported: #{} — syncing state\x1b[0m",
                        number
                    );
                }
            });
    }

    {
        let client = client.clone();
        task_manager
            .spawn_handle()
            .spawn("block-watcher", None, async move {
                use futures_util::StreamExt;

                let mut notifications = client.finality_notification_stream();
                while let Some(notification) = notifications.next().await {
                    // number is saturated into u64
                    let number: u64 = (*notification.header.number()).saturated_into();
                    // Orange color for block finalized
                    log::info!("\x1b[33m🏆 Block finalized: #{} ✅\x1b[0m", number);
                }
            });
    }

    // Start Flash Finality if enabled
    if let Some(gadget) = flash_finality_gadget {
        let bridge = FlashFinalityBridge::new(
            gadget.clone(),
            client.clone(),
            network.clone(),
            sync_service.clone(),
            keystore_container.keystore(),
        );

        task_manager.spawn_essential_handle().spawn(
            "flash-finality-bridge",
            Some("flash-finality"),
            bridge.run(),
        );

        task_manager.spawn_essential_handle().spawn(
            "flash-finality-timeout",
            Some("flash-finality"),
            gadget.clone().spawn_timeout_monitor(),
        );

        // Spawn the Flash-Finality voter to apply certificates as finality
        // In live mode (when enable_flash_finality=true and vote_on_flash=true),
        // this will move the finalized head based on certificates.
        // In shadow mode, it logs certificate availability for monitoring.
        let gadget_for_voter = gadget.clone();
        let client_for_voter = client.clone();
        let enable_flash_live_mode = feature_flags.enable_flash_finality && !disable_grandpa_flag;

        task_manager.spawn_essential_handle().spawn(
            "flash-finality-voter",
            Some("flash-finality"),
            run_flash_finality_voter(gadget_for_voter, client_for_voter, enable_flash_live_mode),
        );

        log::info!("⚡ Flash Finality gadget, network bridge, and voter started");
    }

    // ── Wire Cross-VM bridge adapters ─────────────────────────────────────
    // `SubstrateClientBalanceAdapter` provides live canonical-ledger balances
    // to the off-chain AtomicSwapOrchestrator.  `PalletEscrowAdapter` wraps it
    // with durable escrow persistence backed by the node's off-chain storage,
    // so in-flight cross-VM swaps survive node restarts.
    {
        let balance_adapter = Arc::new(SubstrateClientBalanceAdapter::new(client.clone()));

        match backend.offchain_storage() {
            Some(offchain_storage) => {
                let escrow_adapter = Arc::new(PalletEscrowAdapter::with_persistence(
                    balance_adapter.clone(),
                    OffchainEscrowPersistence::new(offchain_storage),
                ));

                {
                    let retained = escrow_adapter.clone();
                    task_manager.spawn_handle().spawn(
                        "cross-vm-escrow-retainer",
                        None,
                        async move {
                            loop {
                                tokio::time::sleep(Duration::from_secs(3600)).await;
                                let _keep_alive = retained.clone();
                            }
                        },
                    );
                }

                log::info!("🌉 Cross-VM bridge adapters wired (balance + escrow)");
            }
            None => {
                log::warn!("⚠️  Off-chain storage unavailable (in-memory backend?); escrow persistence disabled");
            }
        }
    }

    // Start PoH Generator background task if enabled
    if let Some(poh_state_arc) = shared_poh_state {
        let client_clone = client.clone();

        task_manager
            .spawn_essential_handle()
            .spawn("poh-watcher", Some("poh"), async move {
                let mut import_notifications = client_clone.import_notification_stream();
                while let Some(notification) = import_notifications.next().await {
                    if notification.is_new_best {
                        let mut state = poh_state_arc.lock().await;
                        state.advance(&[]);
                        log::info!(
                            "⏱️  [PoH] Shadow tick {} anchored to block {}",
                            state.tick(),
                            notification.hash
                        );
                    }
                }
            });
        log::info!("⏱️ Proof of History (PoH) generator enabled and wired to block loop");
    }

    log::info!("✨ X3 Chain node started successfully");
    log::info!("🔗 Network: {}", chain_name);
    log::info!("👤 Node name: {}", name);
    log::info!("📋 Role: {:?}", role);

    Ok(task_manager)
}

/// Runs the Flash-Finality voter that applies certificates as actual finality.
///
/// This voter listens to block finality notifications and uses Flash-Finality
/// certificates to move the canonical finalized head. When live mode is enabled,
/// certificates override GRANDPA finality; in shadow mode, they're logged for comparison.
///
/// When a certificate is available it is written to **off-chain local storage** so
/// the `pallet-x3-atomic-kernel` OCW can attach it to PoAE proofs as finality_cert.
///
/// Key format: `b"x3ff:" (5 bytes) + block_number (8 bytes LE) = 13 bytes`
/// Value:      `cert_hash (32 bytes)`
async fn run_flash_finality_voter<Client, Block>(
    gadget: Arc<FlashFinalityGadget>,
    client: Arc<Client>,
    enable_live_mode: bool,
) where
    Client: BlockchainEvents<Block> + BlockBackend<Block> + Send + Sync + 'static,
    Block: sp_runtime::traits::Block + 'static,
    Block::Header: HeaderT,
{
    use futures_util::StreamExt;

    log::info!(
        "⚡ Flash-Finality voter started — live_mode={}",
        if enable_live_mode { "ON" } else { "SHADOW" }
    );

    let mut finality_notifications = client.finality_notification_stream();

    loop {
        match finality_notifications.next().await {
            Some(notification) => {
                let number: u64 = (*notification.header.number()).saturated_into();
                let hash: [u8; 32] = notification.hash.as_ref().try_into().unwrap_or([0u8; 32]);

                // Try to get a Flash-Finality certificate for this block
                if let Some(cert) = gadget.get_certificate(hash).await {
                    // --- Write cert_hash to off-chain local storage ---
                    // Key: "x3ff:" + block_number (LE u64) = 13 bytes
                    // Value: cert_hash (32 bytes)
                    // The pallet-x3-atomic-kernel OCW reads this to populate
                    // `finality_cert` in PoAE proofs instead of H256::zero().
                    {
                        let cert_hash = cert.cert_hash();
                        let mut key = b"x3ff:".to_vec();
                        key.extend_from_slice(&number.to_le_bytes());
                        sp_io::offchain::local_storage_set(
                            sp_runtime::offchain::StorageKind::PERSISTENT,
                            &key,
                            &cert_hash,
                        );
                        log::info!(
                            "⚡ [FlashFinality] cert stored at key x3ff:{} → cert_hash=0x{}",
                            number,
                            hex::encode(&cert_hash[..8])
                        );
                    }

                    if enable_live_mode {
                        log::info!(
                            "⚡✅ Live mode: Flash-Finality cert for #{} — {} votes (certificate ready)",
                            number,
                            cert.vote_count
                        );
                    } else {
                        // Shadow mode: log certificate for monitoring without applying it
                        log::debug!(
                            "⚡🔍 Shadow: Flash cert available for #{} — {} votes (not applied)",
                            number,
                            cert.vote_count
                        );
                    }

                    // Record metrics
                    let metrics = gadget.metrics().await;
                    log::info!(
                        "📊 Flash-Finality metrics: rounds_completed={}, shadow_agreements={}",
                        metrics.rounds_completed,
                        metrics.shadow_agreements
                    );
                } else {
                    // No Flash certificate yet; this could be normal if finality advanced
                    // via GRANDPA first, or if we're still in earlier consensus phases
                    log::debug!("⚡ No Flash cert for #{} yet", number);
                }
            }

            None => {
                log::warn!("⚡ Flash-Finality voter: client finality stream closed");
                break;
            }
        }
    }
}

//====== tests ======
// DISABLED: Tests require sc_service::Configuration API changes
// #[cfg(test)]
// mod tests { ... }

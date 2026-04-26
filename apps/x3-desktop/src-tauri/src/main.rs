#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

mod social;
mod crm;
mod admin_commands;
mod wallet;
mod wallet_core;

use chrono::Utc;
use rand::Rng;
use serde::{Serialize, Deserialize};
use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tauri::{AppHandle, Builder, Emitter, Manager, State, generate_handler};
use tokio::time::sleep;
use sysinfo::System;
use uuid::Uuid;

const TELEMETRY_EVENT: &str = "telemetry_update";
const IPFS_LOCAL: &str = "http://127.0.0.1:5001";

#[derive(Debug, Serialize)]
struct IpcError {
  code: &'static str,
  message: String,
  details: Option<String>,
}

impl IpcError {
  fn new(code: &'static str, message: &str, details: Option<String>) -> Self {
    Self {
      code,
      message: message.to_string(),
      details,
    }
  }
}

impl fmt::Display for IpcError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}: {}", self.code, self.message)
  }
}

impl std::error::Error for IpcError {}

/* ─── System Metrics ─────────────────────────────── */

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SystemMetrics {
  cpu: CpuMetrics,
  memory: MemoryMetrics,
  disk: Vec<DiskMetrics>,
  updated_at: String,
}

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CpuMetrics {
  usage_percent: f32,
  cores: u32,
  frequency: u64,
}

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryMetrics {
  used: u64,
  total: u64,
  usage_percent: f32,
}

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiskMetrics {
  name: String,
  used: u64,
  total: u64,
  usage_percent: f32,
}

/* ─── IPFS Storage & Filecoin-type System ────────── */

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpfsStorageData {
  node_id: String,
  pinned_objects: Vec<PinnedContent>,
  storage_used: u64,
  storage_capacity: u64,
  storage_market: Vec<StorageDeal>,
  total_pins: u32,
  updated_at: String,
}

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PinnedContent {
  cid: String,
  name: String,
  size: u64,
  pinned_at: String,
  replicas: u32,
  earning_potential: f64,
}

#[derive(Serialize, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StorageDeal {
  id: String,
  client: String,
  size: u64,
  price_per_epoch: f64,
  duration_epochs: u32,
  status: StorageDealStatus,
  earned: f64,
}

#[derive(Serialize, Clone, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
enum StorageDealStatus {
  Active,
  Pending,
  Expired,
  Slashed,
}

#[derive(Clone)]
struct TelemetryState {
  swarm: Arc<RwLock<SwarmHealthData>>,
  network: Arc<RwLock<NetworkControlData>>,
  storage: Arc<RwLock<StorageMonitorData>>,
  ide: Arc<RwLock<IdeTelemetryData>>,
  system: Arc<RwLock<SystemMetrics>>,
  ipfs: Arc<RwLock<IpfsStorageData>>,
  /// Reusable sysinfo handle — creating System::new_all() is very expensive;
  /// we keep one instance and call refresh_*() on it each tick instead.
  sys_handle: Arc<std::sync::Mutex<System>>,
}

impl TelemetryState {
  fn new() -> Self {
    let mut sys = System::new_all();
    sys.refresh_all();
    let initial_metrics = read_system_metrics(&sys);
    Self {
      swarm: Arc::new(RwLock::new(seed_swarm_health())),
      network: Arc::new(RwLock::new(seed_network_control())),
      storage: Arc::new(RwLock::new(seed_storage_monitor())),
      ide: Arc::new(RwLock::new(seed_ide_telemetry())),
      system: Arc::new(RwLock::new(initial_metrics)),
      ipfs: Arc::new(RwLock::new(seed_ipfs_storage())),
      sys_handle: Arc::new(std::sync::Mutex::new(sys)),
    }
  }

  fn snapshot(&self) -> TelemetrySnapshot {
    TelemetrySnapshot {
      swarm: self.swarm.read().expect("swarm read lock").clone(),
      network: self.network.read().expect("network read lock").clone(),
      storage: self.storage.read().expect("storage read lock").clone(),
      ide: self.ide.read().expect("ide read lock").clone(),
      updated_at: Utc::now().to_rfc3339(),
    }
  }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TelemetrySnapshot {
  swarm: SwarmHealthData,
  network: NetworkControlData,
  storage: StorageMonitorData,
  ide: IdeTelemetryData,
  updated_at: String,
}

/* ─── Swarm health payloads ─────────────────────── */

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SwarmHealthData {
  summary: SwarmSummary,
  nodes: Vec<SwarmNode>,
  updated_at: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SwarmSummary {
  online_nodes: u32,
  total_nodes: u32,
  avg_gpu_util: f32,
  total_vram_used: u64,
  total_vram_capacity: u64,
  queued_jobs: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SwarmNode {
  id: String,
  name: String,
  status: NodeStatus,
  gpu_util: f32,
  vram_used: u64,
  vram_capacity: u64,
  temperature: f32,
  uptime_hours: u32,
  sla: u8,
  jobs: u8,
}

#[derive(Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum NodeStatus {
  Online,
  Idle,
  Offline,
  Slashed,
}

#[tauri::command]
fn launch_swarm_health(state: State<TelemetryState>) -> Result<SwarmHealthData, IpcError> {
  // TODO: replace with tauri-plugin-system-info + GPU job queue service
  Ok(state.swarm.read().expect("swarm read lock").clone())
}

fn seed_swarm_health() -> SwarmHealthData {
  let nodes = vec![
    SwarmNode {
      id: "node-0".into(),
      name: "x3-gpu-0".into(),
      status: NodeStatus::Online,
      gpu_util: 72.0,
      vram_used: 18_400_000,
      vram_capacity: 24_576_000,
      temperature: 64.0,
      uptime_hours: 412,
      sla: 99,
      jobs: 6,
    },
    SwarmNode {
      id: "node-1".into(),
      name: "x3-gpu-1".into(),
      status: NodeStatus::Online,
      gpu_util: 59.0,
      vram_used: 16_400_000,
      vram_capacity: 24_576_000,
      temperature: 61.0,
      uptime_hours: 208,
      sla: 98,
      jobs: 4,
    },
    SwarmNode {
      id: "node-2".into(),
      name: "edge-node-a".into(),
      status: NodeStatus::Idle,
      gpu_util: 18.0,
      vram_used: 3_220_000,
      vram_capacity: 12_288_000,
      temperature: 48.0,
      uptime_hours: 36,
      sla: 95,
      jobs: 1,
    },
    SwarmNode {
      id: "node-3".into(),
      name: "cloud-rtx-0".into(),
      status: NodeStatus::Slashed,
      gpu_util: 33.0,
      vram_used: 7_400_000,
      vram_capacity: 24_576_000,
      temperature: 71.0,
      uptime_hours: 120,
      sla: 84,
      jobs: 0,
    },
  ];

  SwarmHealthData {
    summary: summarize_swarm(&nodes),
    nodes,
    updated_at: Utc::now().to_rfc3339(),
  }
}

fn summarize_swarm(nodes: &[SwarmNode]) -> SwarmSummary {
  let total_nodes = nodes.len() as u32;
  let online_nodes = nodes.iter().filter(|n| matches!(n.status, NodeStatus::Online)).count() as u32;
  let avg_gpu_util = nodes.iter().map(|n| n.gpu_util).sum::<f32>() / (nodes.len() as f32).max(1.0);
  let total_vram_used = nodes.iter().map(|n| n.vram_used).sum();
  let total_vram_capacity = nodes.iter().map(|n| n.vram_capacity).sum();
  let queued_jobs = nodes.iter().map(|n| n.jobs as u32).sum();

  SwarmSummary {
    online_nodes,
    total_nodes,
    avg_gpu_util,
    total_vram_used,
    total_vram_capacity,
    queued_jobs,
  }
}

/* ─── Network control payloads ─────────────────── */

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetworkControlData {
  peers: Vec<NetworkPeer>,
  rpc_endpoints: Vec<NetworkRpcEndpoint>,
  logs: Vec<NetworkLogEntry>,
  updated_at: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetworkPeer {
  id: String,
  addr: String,
  protocol: String,
  latency_ms: u32,
  status: PeerStatus,
  last_seen_seconds: u32,
  bytes_sent: u64,
  bytes_received: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetworkRpcEndpoint {
  name: String,
  url: String,
  status: EndpointStatus,
  calls: u32,
  avg_ms: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct NetworkLogEntry {
  ts: String,
  level: LogLevel,
  message: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum PeerStatus {
  Connected,
  Stale,
  Disconnected,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum EndpointStatus {
  Active,
  Degraded,
  Down,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum LogLevel {
  Info,
  Warn,
  Error,
}

#[tauri::command]
fn launch_network_control(state: State<TelemetryState>) -> Result<NetworkControlData, IpcError> {
  // TODO: replace with taurpc stack (tcp/udp/mqtt) + RPC instrumentation
  Ok(state.network.read().expect("network read lock").clone())
}

fn seed_network_control() -> NetworkControlData {
  NetworkControlData {
    peers: vec![
      NetworkPeer {
        id: "peer-0".into(),
        addr: "127.0.0.1:30333".into(),
        protocol: "tcp".into(),
        latency_ms: 12,
        status: PeerStatus::Connected,
        last_seen_seconds: 1,
        bytes_sent: 10_482_221,
        bytes_received: 54_842_113,
      },
      NetworkPeer {
        id: "peer-1".into(),
        addr: "relay.x3-chain.io:443".into(),
        protocol: "ws".into(),
        latency_ms: 33,
        status: PeerStatus::Connected,
        last_seen_seconds: 2,
        bytes_sent: 2_003_112,
        bytes_received: 6_124_900,
      },
      NetworkPeer {
        id: "peer-2".into(),
        addr: "10.0.0.5:9944".into(),
        protocol: "ws".into(),
        latency_ms: 0,
        status: PeerStatus::Disconnected,
        last_seen_seconds: 342,
        bytes_sent: 0,
        bytes_received: 0,
      },
    ],
    rpc_endpoints: vec![
      NetworkRpcEndpoint {
        name: "X3 Kernel RPC".into(),
        url: "127.0.0.1:9944".into(),
        status: EndpointStatus::Active,
        calls: 14_203,
        avg_ms: 12,
      },
      NetworkRpcEndpoint {
        name: "Swarm Coordinator".into(),
        url: "127.0.0.1:8080".into(),
        status: EndpointStatus::Active,
        calls: 3_891,
        avg_ms: 28,
      },
      NetworkRpcEndpoint {
        name: "Storage Gateway".into(),
        url: "127.0.0.1:5001".into(),
        status: EndpointStatus::Degraded,
        calls: 921,
        avg_ms: 145,
      },
    ],
    logs: vec![
      NetworkLogEntry {
        ts: "14:37:12".into(),
        level: LogLevel::Info,
        message: "Peer x3-gpu-0 connected (tcp)".into(),
      },
      NetworkLogEntry {
        ts: "14:37:29".into(),
        level: LogLevel::Warn,
        message: "Heartbeat latency spike: 172ms".into(),
      },
      NetworkLogEntry {
        ts: "14:37:53".into(),
        level: LogLevel::Info,
        message: "RPC trace chain_getBlockHash: 12ms".into(),
      },
    ],
    updated_at: Utc::now().to_rfc3339(),
  }
}

/* ─── Storage monitor payloads ─────────────────── */

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StorageMonitorData {
  pins: Vec<StoragePin>,
  proofs: Vec<StorageProof>,
  capacity_bytes: u64,
  used_bytes: u64,
  updated_at: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StoragePin {
  cid: String,
  name: String,
  size: u64,
  status: StoragePinStatus,
  replicas: u32,
  proof_age_minutes: u32,
  r#type: StoragePinType,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StorageProof {
  cid: String,
  epoch: u64,
  result: StorageProofResult,
  timestamp: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum StoragePinStatus {
  Pinned,
  Pinning,
  Unpinned,
  Failed,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum StoragePinType {
  Snapshot,
  Artifact,
  AgentMemory,
  Contract,
  Dataset,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum StorageProofResult {
  Valid,
  Challenged,
  Expired,
}

#[tauri::command]
fn launch_storage_monitor(state: State<TelemetryState>) -> Result<StorageMonitorData, IpcError> {
  // TODO: replace with tauri-plugin-fs / OTA proofs pipeline data
  Ok(state.storage.read().expect("storage read lock").clone())
}

fn seed_storage_monitor() -> StorageMonitorData {
  let pins = vec![
    StoragePin {
      cid: "bafy2bza...k3f9x".into(),
      name: "runtime-wasm-v0.8.2".into(),
      size: 4_812_300,
      status: StoragePinStatus::Pinned,
      replicas: 5,
      proof_age_minutes: 3,
      r#type: StoragePinType::Artifact,
    },
    StoragePin {
      cid: "bafy2bza...m7p2q".into(),
      name: "agent-memory-alpha.snap".into(),
      size: 18_432_000,
      status: StoragePinStatus::Pinned,
      replicas: 3,
      proof_age_minutes: 12,
      r#type: StoragePinType::AgentMemory,
    },
    StoragePin {
      cid: "bafy2bza...a8c2e".into(),
      name: "training-data-v3.tar".into(),
      size: 1_073_741_824,
      status: StoragePinStatus::Failed,
      replicas: 0,
      proof_age_minutes: 999,
      r#type: StoragePinType::Dataset,
    },
  ];

  let proofs = vec![
    StorageProof {
      cid: "bafy2bza...k3f9x".into(),
      epoch: 1_284_391,
      result: StorageProofResult::Valid,
      timestamp: "14:32:01".into(),
    },
    StorageProof {
      cid: "bafy2bza...v9s1r".into(),
      epoch: 1_284_391,
      result: StorageProofResult::Valid,
      timestamp: "14:31:58".into(),
    },
    StorageProof {
      cid: "bafy2bza...t6n3y".into(),
      epoch: 1_284_390,
      result: StorageProofResult::Challenged,
      timestamp: "14:28:44".into(),
    },
  ];

  StorageMonitorData {
    pins,
    proofs,
    capacity_bytes: 20 * 1_073_741_824,
    used_bytes: 8_406_643_200,
    updated_at: Utc::now().to_rfc3339(),
  }
}

/* ─── IDE telemetry payloads ────────────────────── */

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct IdeTelemetryData {
  builds: Vec<BuildJob>,
  contracts: Vec<IdeContract>,
  traces: Vec<TraceEntry>,
  log_lines: Vec<String>,
  updated_at: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct BuildJob {
  id: String,
  target: String,
  status: BuildStatus,
  duration_seconds: u32,
  timestamp: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct IdeContract {
  name: String,
  address: String,
  network: String,
  status: IdeContractStatus,
  gas_used: u64,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TraceEntry {
  block_num: u64,
  extrinsic: String,
  result: TraceResult,
  gas_used: u64,
  state_root: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum BuildStatus {
  Building,
  Success,
  Failed,
  Queued,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum IdeContractStatus {
  Deployed,
  Pending,
  Failed,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
enum TraceResult {
  Ok,
  Err,
}

#[tauri::command]
fn launch_ide_ipc(state: State<TelemetryState>) -> Result<IdeTelemetryData, IpcError> {
  // TODO: wire RPC / auth telemetry pipeline (builds, contracts, trace logs)
  Ok(state.ide.read().expect("ide read lock").clone())
}

#[tauri::command]
fn launch_system_metrics(state: State<TelemetryState>) -> Result<SystemMetrics, IpcError> {
  Ok(state.system.read().expect("system read lock").clone())
}

#[tauri::command]
fn launch_ipfs_storage(state: State<TelemetryState>) -> Result<IpfsStorageData, IpcError> {
  Ok(state.ipfs.read().expect("ipfs read lock").clone())
}

fn seed_ide_telemetry() -> IdeTelemetryData {
  IdeTelemetryData {
    builds: vec![
      BuildJob {
        id: "b-1".into(),
        target: "x3-chain-runtime".into(),
        status: BuildStatus::Success,
        duration_seconds: 142,
        timestamp: "14:28:03".into(),
      },
      BuildJob {
        id: "b-2".into(),
        target: "x3-lang-stdlib v0.3.0".into(),
        status: BuildStatus::Building,
        duration_seconds: 0,
        timestamp: "14:32:18".into(),
      },
    ],
    contracts: vec![
      IdeContract {
        name: "HTLC_v2".into(),
        address: "5GrwvaEF...43jS".into(),
        network: "x3-testnet".into(),
        status: IdeContractStatus::Deployed,
        gas_used: 2_480_000,
      },
      IdeContract {
        name: "GovernanceProxy".into(),
        address: "5FHneW46...8qPm".into(),
        network: "x3-testnet".into(),
        status: IdeContractStatus::Deployed,
        gas_used: 1_120_000,
      },
    ],
    traces: vec![
      TraceEntry {
        block_num: 1_284_391,
        extrinsic: "Balances::transfer".into(),
        result: TraceResult::Ok,
        gas_used: 125_000,
        state_root: "0xa3f2...d891".into(),
      },
      TraceEntry {
        block_num: 1_284_390,
        extrinsic: "HTLC::claim".into(),
        result: TraceResult::Ok,
        gas_used: 210_000,
        state_root: "0xc9d3...f103".into(),
      },
    ],
    log_lines: vec![
      "Compiling x3-chain-runtime v0.8.2".into(),
      "Compiling pallet-swarm v0.4.1".into(),
      "Building [===========>        ] 58%".into(),
    ],
    updated_at: Utc::now().to_rfc3339(),
  }
}

fn start_mock_stream(app: AppHandle, state: TelemetryState) {
  tauri::async_runtime::spawn(async move {
    loop {
      sleep(Duration::from_millis(3000)).await;
      let mut rng = rand::thread_rng();
      update_swarm(&state, &mut rng);
      update_network(&state, &mut rng);
      update_storage(&state, &mut rng);
      update_ide(&state, &mut rng);
      update_system_metrics(&state);
      update_ipfs_storage(&state, &mut rng);

      let snapshot = state.snapshot();
      let _ = app.emit(TELEMETRY_EVENT, snapshot);
    }
  });
}

/* ─── Real System Monitoring ─────────────────────── */

/// Read metrics from an already-refreshed System handle (cheap).
fn read_system_metrics(sys: &System) -> SystemMetrics {
  let cpu_info = sys.global_cpu_info();
  let cpu_usage = cpu_info.cpu_usage();
  let total_memory = sys.total_memory();
  let used_memory = sys.used_memory();

  SystemMetrics {
    cpu: CpuMetrics {
      usage_percent: cpu_usage,
      cores: sys.cpus().len() as u32,
      frequency: sys.cpus().first().map(|c| c.frequency()).unwrap_or(0),
    },
    memory: MemoryMetrics {
      used: used_memory * 1024,
      total: total_memory * 1024,
      usage_percent: if total_memory > 0 {
        (used_memory as f32 / total_memory as f32) * 100.0
      } else {
        0.0
      },
    },
    disk: vec![
      DiskMetrics {
        name: "System".into(),
        used: used_memory * 1024,
        total: total_memory * 1024,
        usage_percent: if total_memory > 0 {
          (used_memory as f32 / total_memory as f32) * 100.0
        } else {
          0.0
        },
      }
    ],
    updated_at: Utc::now().to_rfc3339(),
  }
}

fn update_system_metrics(state: &TelemetryState) {
  let mut sys = state.sys_handle.lock().expect("sys_handle lock");
  sys.refresh_cpu();
  sys.refresh_memory();
  let metrics = read_system_metrics(&sys);
  drop(sys); // release mutex before acquiring RwLock
  *state.system.write().expect("system write lock") = metrics;
}

/* ─── IPFS Storage & Filecoin-type System ────────── */

fn seed_ipfs_storage() -> IpfsStorageData {
  IpfsStorageData {
    node_id: Uuid::new_v4().to_string(),
    pinned_objects: vec![
      PinnedContent {
        cid: "bafy2bzaceayutrxdyedzv2n7yguwq4py2w4xfa2z4aceo4vq3bsfzb5zraea".into(),
        name: "x3-runtime.wasm".into(),
        size: 4_812_300,
        pinned_at: Utc::now().to_rfc3339(),
        replicas: 5,
        earning_potential: 150.50,
      },
      PinnedContent {
        cid: "bafy2bzaceayp7fq2kmx3vhyikpohczb73f7mw7bnvp6u4zvvqfqiduxpvrhq".into(),
        name: "training-data.tar".into(),
        size: 18_432_000,
        pinned_at: (Utc::now() - chrono::Duration::days(2)).to_rfc3339(),
        replicas: 3,
        earning_potential: 420.75,
      },
    ],
    storage_used: 4_812_300 + 18_432_000,
    storage_capacity: 500_000_000_000,  // 500GB
    storage_market: vec![
      StorageDeal {
        id: "deal-001".into(),
        client: "x3-ai-lab".into(),
        size: 4_812_300,
        price_per_epoch: 0.5,
        duration_epochs: 520,
        status: StorageDealStatus::Active,
        earned: 260.0,
      },
      StorageDeal {
        id: "deal-002".into(),
        client: "research-collective".into(),
        size: 18_432_000,
        price_per_epoch: 1.25,
        duration_epochs: 260,
        status: StorageDealStatus::Active,
        earned: 325.0,
      },
    ],
    total_pins: 2,
    updated_at: Utc::now().to_rfc3339(),
  }
}

fn update_ipfs_storage(state: &TelemetryState, rng: &mut impl Rng) {
  let mut data = state.ipfs.write().expect("ipfs write lock");
  
  // Simulate storage earning updates
  for deal in data.storage_market.iter_mut() {
    if matches!(deal.status, StorageDealStatus::Active) {
      deal.earned += deal.price_per_epoch * rng.gen_range(0.8..1.2);
    }
  }
  
  // Simulate new replicas
  for pin in data.pinned_objects.iter_mut() {
    if rng.gen_bool(0.2) && pin.replicas < 10 {
      pin.replicas += 1;
      pin.earning_potential += 25.0;
    }
  }
  
  data.storage_used = data.pinned_objects.iter().map(|p| p.size).sum();
  data.updated_at = Utc::now().to_rfc3339();
}

fn update_swarm(state: &TelemetryState, rng: &mut impl Rng) {
  let mut data = state.swarm.write().expect("swarm write lock");

  for node in data.nodes.iter_mut() {
    if matches!(node.status, NodeStatus::Offline) {
      node.gpu_util = 0.0;
      node.temperature = 0.0;
      node.jobs = 0;
      continue;
    }

    node.gpu_util = clamp_f32(node.gpu_util + jitter(rng, 6.0), 0.0, 100.0);
    node.temperature = clamp_f32(42.0 + node.gpu_util * 0.4 + jitter(rng, 2.0), 40.0, 92.0);

    let vram_delta = jitter(rng, 1_200_000.0) as i64;
    node.vram_used = clamp_u64_signed(node.vram_used as i64 + vram_delta, 0, node.vram_capacity as i64) as u64;

    if node.status == NodeStatus::Online {
      let delta = rng.gen_range(-1..=2) as i16;
      node.jobs = clamp_u8_signed(node.jobs as i16 + delta, 0, 12);
    }

    if node.sla > 80 {
      node.sla = clamp_u8_signed(node.sla as i16 + rng.gen_range(-1..=1), 80, 100);
    }
  }

  data.summary = summarize_swarm(&data.nodes);
  data.updated_at = Utc::now().to_rfc3339();
}

fn update_network(state: &TelemetryState, rng: &mut impl Rng) {
  let mut data = state.network.write().expect("network write lock");
  for peer in data.peers.iter_mut() {
    if matches!(peer.status, PeerStatus::Disconnected) {
      continue;
    }

    peer.latency_ms = clamp_u32_signed(peer.latency_ms as i64 + rng.gen_range(-8..=12), 5, 240);
    peer.bytes_sent = peer.bytes_sent.saturating_add(rng.gen_range(10_000..80_000));
    peer.bytes_received = peer.bytes_received.saturating_add(rng.gen_range(15_000..120_000));
    peer.last_seen_seconds = rng.gen_range(0..=6);
  }

  if let Some(endpoint) = data.rpc_endpoints.first_mut() {
    endpoint.calls += rng.gen_range(8..40);
    endpoint.avg_ms = clamp_u32_signed(endpoint.avg_ms as i64 + rng.gen_range(-3..=5), 8, 120);
  }

  let log_messages = [
    "Gossip heartbeat received",
    "RPC batch latency spike",
    "Peer x3-gpu-2 reconnected",
    "Storage gateway ping OK",
    "Dispatch queue drained",
  ];
  let entry = NetworkLogEntry {
    ts: Utc::now().format("%H:%M:%S").to_string(),
    level: LogLevel::Info,
    message: log_messages[rng.gen_range(0..log_messages.len())].into(),
  };

  data.logs.push(entry);
  let log_len = data.logs.len();
  if log_len > 50 {
    let drain_count = log_len - 50;
    data.logs.drain(0..drain_count);
  }

  data.updated_at = Utc::now().to_rfc3339();
}

fn update_storage(state: &TelemetryState, rng: &mut impl Rng) {
  let mut data = state.storage.write().expect("storage write lock");
  let delta = jitter(rng, 450_000_000.0) as i64;
  let next_used = clamp_u64_signed(data.used_bytes as i64 + delta, 0, data.capacity_bytes as i64);
  data.used_bytes = next_used as u64;

  for pin in data.pins.iter_mut() {
    pin.proof_age_minutes = pin.proof_age_minutes.saturating_add(rng.gen_range(0..=2));
  }

  data.updated_at = Utc::now().to_rfc3339();
}

fn update_ide(state: &TelemetryState, rng: &mut impl Rng) {
  let mut data = state.ide.write().expect("ide write lock");

  for build in data.builds.iter_mut() {
    if matches!(build.status, BuildStatus::Building) {
      build.duration_seconds += rng.gen_range(1..4);
    }
  }

  let log_snippets = [
    "Compiling pallet-storage v0.5.0",
    "Linking x3-chain-runtime",
    "Finished release build",
    "Replay trace exported",
  ];
  data.log_lines.push(log_snippets[rng.gen_range(0..log_snippets.len())].into());
  let log_line_len = data.log_lines.len();
  if log_line_len > 12 {
    let drain_count = log_line_len - 12;
    data.log_lines.drain(0..drain_count);
  }

  data.updated_at = Utc::now().to_rfc3339();
}

fn jitter(rng: &mut impl Rng, range: f32) -> f32 {
  rng.gen_range(-range..=range)
}

fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
  value.max(min).min(max)
}

fn clamp_u32_signed(value: i64, min: u32, max: u32) -> u32 {
  value.max(min as i64).min(max as i64) as u32
}

fn clamp_u64_signed(value: i64, min: i64, max: i64) -> i64 {
  value.max(min).min(max)
}

fn clamp_u8_signed(value: i16, min: u8, max: u8) -> u8 {
  value.max(min as i16).min(max as i16) as u8
}

/* ─── Application Registry ──────────────────────── */

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Application {
  id: String,
  name: String,
  description: String,
  category: String,
}

#[tauri::command]
fn get_app_registry() -> Result<Vec<Application>, IpcError> {
  // Return an empty array; the frontend will use its default registry
  // This handler exists primarily to signal the backend is alive
  Ok(vec![])
}

fn main() {
  let telemetry_state = TelemetryState::new();

  Builder::default()
    .manage(telemetry_state.clone())
    .invoke_handler(generate_handler![
      get_app_registry,
      launch_swarm_health,
      launch_network_control,
      launch_storage_monitor,
      launch_ide_ipc,
      launch_system_metrics,
      launch_ipfs_storage,
      admin_commands::run_system_command,
      // ── Social Network ──
      social::commands::social_register,
      social::commands::social_login,
      social::commands::social_logout,
      social::commands::social_get_profile,
      social::commands::social_get_profile_by_username,
      social::commands::social_update_profile,
      social::commands::social_send_friend_request,
      social::commands::social_respond_friend_request,
      social::commands::social_get_friends,
      social::commands::social_get_pending_requests,
      social::commands::social_set_top_friends,
      social::commands::social_remove_friend,
      social::commands::social_send_message,
      social::commands::social_get_inbox,
      social::commands::social_get_sent_messages,
      social::commands::social_mark_message_read,
      social::commands::social_delete_message,
      social::commands::social_post_bulletin,
      social::commands::social_get_bulletins,
      social::commands::social_post_comment,
      social::commands::social_get_comments,
      social::commands::social_delete_comment,
      social::commands::social_create_blog_post,
      social::commands::social_get_blog_posts,
      social::commands::social_post_blog_comment,
      social::commands::social_add_photo,
      social::commands::social_get_photos,
      social::commands::social_delete_photo,
      social::commands::social_add_music,
      social::commands::social_get_music,
      social::commands::social_set_profile_song,
      social::commands::social_post_status,
      social::commands::social_get_feed,
      social::commands::social_search_users,
      social::commands::social_send_kudo,
      social::commands::social_get_kudos,
      social::commands::social_create_group,
      social::commands::social_get_groups,
      social::commands::social_join_group,
      social::commands::social_get_stats,
      social::commands::social_browse_users,
      social::commands::social_get_team_codes,
      social::commands::social_create_team_code,
      social::commands::social_validate_team_code,
      // ── CRM ──
      crm::commands::crm_create_contact,
      crm::commands::crm_update_contact,
      crm::commands::crm_get_contacts,
      crm::commands::crm_get_contact,
      crm::commands::crm_delete_contact,
      crm::commands::crm_create_event,
      crm::commands::crm_update_event,
      crm::commands::crm_get_events,
      crm::commands::crm_delete_event,
      crm::commands::crm_create_deal,
      crm::commands::crm_update_deal,
      crm::commands::crm_get_deals,
      crm::commands::crm_delete_deal,
      crm::commands::crm_create_activity,
      crm::commands::crm_get_activities,
      crm::commands::crm_create_email_template,
      crm::commands::crm_get_email_templates,
      crm::commands::crm_delete_email_template,
      crm::commands::crm_save_smtp_config,
      crm::commands::crm_get_smtp_config,
      crm::commands::crm_send_email,
      crm::commands::crm_get_sent_emails,
      crm::commands::crm_get_stats,
      // ── CRM Agents (AI-powered marketing team) ──
      crm::agents::agents_get_roster,
      crm::agents::agents_check_status,
      crm::agents::agents_run_task,
      crm::agents::agents_chat,
      crm::agents::agents_get_history,
      crm::agents::agents_get_tasks,
      crm::agents::agents_create_lead,
      crm::agents::agents_update_lead,
      crm::agents::agents_get_leads,
      crm::agents::agents_assign_email,
      crm::agents::agents_get_user_email,
      crm::agents::agents_assign_proxy,
      crm::agents::agents_get_proxy,
      crm::agents::agents_get_all_proxies,
      crm::agents::agents_get_funnel_stats,
      // ── Web Search & Research ──
      crm::agents::agents_web_search,
      crm::agents::agents_fetch_website,
      // ── RAG System ──
      crm::agents::agents_rag_index,
      crm::agents::agents_rag_query,
      crm::agents::agents_rag_stats,
      // ── Contact Import & Sorting ──
      crm::agents::agents_import_contacts,
      crm::agents::agents_get_contacts_sorted,
      crm::agents::agents_get_contact_filters,
      // ── Proxy Toggle ──
      crm::agents::agents_toggle_proxy,
      // ── Media Folder ──
      crm::agents::agents_scan_media,
      crm::agents::agents_get_media,
      // ── Personalized Messages ──
      crm::agents::agents_personalized_message,
      // ── 90-Day Rollout ──
      crm::agents::agents_seed_rollout,
      crm::agents::agents_get_rollout,
      crm::agents::agents_update_rollout,
      // ── Page Builder ──
      crm::agents::agents_generate_page,
      crm::agents::agents_get_pages,
      crm::agents::agents_get_page_content,
      crm::agents::agents_update_page_status,
      crm::agents::agents_delete_page,
      // ── Agent Hierarchy ──
      crm::agents::agents_get_hierarchy,
      // ── Wallet ──
      wallet::generate_universal_wallet,
      wallet::import_universal_wallet,
      wallet::get_evm_chain_count,
      wallet::store_wallet_secure,
      wallet::get_wallet_balance,
      wallet::submit_cross_swap,
      wallet::execute_x3_script,
      wallet::run_cross_chain_intent,
    ])
    .setup(move |app| {
      // Initialize social database
      let app_dir = app.path().app_data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
      let social_db = social::db::SocialDb::new(app_dir.clone())
          .expect("failed to initialize social database");
      app.manage(social_db);
      let crm_db = crm::db::CrmDb::new(app_dir)
          .expect("failed to initialize CRM database");
      app.manage(crm_db);
      start_mock_stream(app.handle().clone(), telemetry_state.clone());
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("failed to run tauri application");
}

#[cfg(test)]
mod tests;

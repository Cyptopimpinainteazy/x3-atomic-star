/**
 * x3ChainService.ts
 *
 * Live connection layer between the X3 Tauri desktop and the X3 blockchain.
 * Uses @polkadot/api to:
 *   - Connect to a running X3 node via WebSocket RPC
 *   - Query the AtomicTradeEngine runtime API (simulate_trade, find_route, get_price_data)
 *   - Submit signed extrinsics (create_trade_batch, execute_trade_batch)
 *   - Subscribe to chain events for real-time trade status
 */

import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
// @polkadot/extension-inject types imported dynamically via extension-dapp

// ─── Constants ────────────────────────────────────────────────────────────────

function hasTauriRuntime(): boolean {
  return typeof window !== 'undefined' && !!(((window as any).__TAURI_INTERNALS__) || ((window as any).__TAURI__));
}

function isLoopbackEndpoint(endpoint?: string): boolean {
  return !!endpoint && /(127\.0\.0\.1|localhost)/i.test(endpoint);
}

function allowLoopbackInBrowser(): boolean {
  return String((import.meta as any).env?.VITE_ALLOW_LOOPBACK_RPC_IN_BROWSER ?? "").toLowerCase() === "true";
}

const PUBLIC_BROWSER_WS_FALLBACK = 'wss://ws.x3star.net/ws';
const BROWSER_RPC_BACKOFF_MS = 30_000;

function isBrowserPreviewRpcMode(): boolean {
  return !hasTauriRuntime() && !allowLoopbackInBrowser();
}

/** Prefer public RPC in browser preview unless a local endpoint is explicitly configured. */
const CONFIGURED_MAINNET_WS = (import.meta.env.VITE_RPC_WS as string) || PUBLIC_BROWSER_WS_FALLBACK;
const SAFE_MAINNET_WS = !hasTauriRuntime() && !allowLoopbackInBrowser() && isLoopbackEndpoint(CONFIGURED_MAINNET_WS)
  ? PUBLIC_BROWSER_WS_FALLBACK
  : CONFIGURED_MAINNET_WS;

const RAW_DEFAULT_WS = import.meta.env.VITE_X3_NODE_WS
  ?? (hasTauriRuntime()
    ? ((import.meta.env.VITE_RPC_WS_LOCAL as string) || 'ws://127.0.0.1:9944')
    : SAFE_MAINNET_WS);

const DEFAULT_WS = !hasTauriRuntime() && !allowLoopbackInBrowser() && isLoopbackEndpoint(RAW_DEFAULT_WS)
  ? SAFE_MAINNET_WS
  : RAW_DEFAULT_WS;

/** Well-known token H256 identifiers (derived from AssetId 0-3 in little-endian) */
export const TOKEN_IDS: Record<string, string> = {
  X3:   '0x0000000000000000000000000000000000000000000000000000000000000000',
  ETH:  '0x0100000000000000000000000000000000000000000000000000000000000000',
  SOL:  '0x0200000000000000000000000000000000000000000000000000000000000000',
  USDC: '0x0300000000000000000000000000000000000000000000000000000000000000',
};

const X3_RPC_TYPES: any = {
  VmType: {
    _enum: ['Evm', 'Svm', 'X3', 'CrossVm'],
  },
  AmmProtocol: {
    _enum: [
      'UniswapV2',
      'UniswapV3',
      'Raydium',
      'OrcaWhirlpool',
      'AtlasAmm',
      'ConstantProduct',
      'StableSwap',
    ],
  },
  RouteStepRpc: {
    pool_id: 'H256',
    token_in: 'H256',
    token_out: 'H256',
    protocol: 'AmmProtocol',
    vm_type: 'VmType',
  },
  TradeRouteRpc: {
    steps: 'Vec<RouteStepRpc>',
    token_start: 'H256',
    token_end: 'H256',
    amount_in: 'u128',
    expected_amount_out: 'u128',
    estimated_gas: 'u64',
    price_impact_bps: 'u32',
  },
  SimulationResultRpc: {
    success: 'bool',
    estimated_output: 'u128',
    price_impact_bps: 'u32',
    evm_gas: 'u64',
    svm_compute: 'u64',
    route: 'Vec<RouteStepRpc>',
    error: 'Option<Bytes>',
  },
  PriceDataResponseRpc: {
    exists: 'bool',
    twap_price: 'Option<u128>',
    latest_price: 'Option<u128>',
    observation_count: 'u32',
    last_updated: 'u64',
  },
  BatchStatusResponseRpc: {
    exists: 'bool',
    status: 'u8',
    submitted_at: 'u64',
    finalized_at: 'Option<u64>',
    legs_executed: 'u32',
    checkpoints: 'u32',
  },
};

const X3_RPC: any = {
  atomicTrade: {
    simulate: {
      description: 'Simulate an atomic trade route',
      params: [
        { name: 'tokenIn', type: 'H256' },
        { name: 'tokenOut', type: 'H256' },
        { name: 'amountIn', type: 'u128' },
        { name: 'slippageBps', type: 'u32' },
        { name: 'at', type: 'BlockHash', isOptional: true },
      ],
      type: 'SimulationResultRpc',
    },
    getPriceData: {
      description: 'Get atomic trade oracle data',
      params: [
        { name: 'tokenA', type: 'H256' },
        { name: 'tokenB', type: 'H256' },
        { name: 'at', type: 'BlockHash', isOptional: true },
      ],
      type: 'PriceDataResponseRpc',
    },
    getBatchStatus: {
      description: 'Get atomic trade batch status',
      params: [
        { name: 'batchId', type: 'H256' },
        { name: 'at', type: 'BlockHash', isOptional: true },
      ],
      type: 'BatchStatusResponseRpc',
    },
    findRoute: {
      description: 'Find the best atomic trade route',
      params: [
        { name: 'tokenIn', type: 'H256' },
        { name: 'tokenOut', type: 'H256' },
        { name: 'amountIn', type: 'u128' },
        { name: 'at', type: 'BlockHash', isOptional: true },
      ],
      type: 'Option<TradeRouteRpc>',
    },
  },
  x3: {
    findBestPath: {
      description: 'Resolve the best DEX path across registered AMMs',
      params: [
        { name: 'tokenIn', type: 'H256' },
        { name: 'tokenOut', type: 'H256' },
        { name: 'amountIn', type: 'u128' },
        { name: 'at', type: 'BlockHash', isOptional: true },
      ],
      type: 'Option<TradeRouteRpc>',
    },
  },
};

/** VM type enum matching the Rust pallet */
export enum VmType {
  EVM   = 0,
  SVM   = 1,
  X3    = 2,
  Cross = 3, // CrossVm
}

// ─── Types ────────────────────────────────────────────────────────────────────

export interface SimulationResult {
  success: boolean;
  estimatedOutput: bigint;
  priceImpactBps: number;
  evmGas: bigint;
  svmCompute: bigint;
  route: RouteStep[];
  error?: string;
}

export interface RouteStep {
  poolId: string;
  tokenIn: string;
  tokenOut: string;
  protocol: string;
  vmType: VmType;
}

export interface BestPathResult {
  steps: RouteStep[];
  expectedOutput: bigint;
  estimatedGas: bigint;
  priceImpactBps: number;
  isCrossVm: boolean;
  hopCount: number;
}

export interface LiquidityPool {
  poolId: string;
  protocol: string;
  vmType: VmType;
  tokenA: string;
  tokenB: string;
  reserveA: bigint;
  reserveB: bigint;
  feeBps: number;
  address: string;
}

export interface TradeLeg {
  vmType: VmType;
  tokenIn: string;
  tokenOut: string;
  amountIn: bigint;
  minAmountOut: bigint;
  deadline: number;
}

export interface TradeReceipt {
  batchId: string;
  status: 'pending' | 'executing' | 'finalized' | 'rolled_back';
  txHash?: string;
  blockNumber?: number;
  legsExecuted: number;
  error?: string;
}

export type SwapStatus =
  | { type: 'idle' }
  | { type: 'simulating' }
  | { type: 'awaiting_signature' }
  | { type: 'submitting' }
  | { type: 'rolling_back'; batchId?: string; failedLegIndex?: number; error?: string }
  | { type: 'finalized'; receipt: TradeReceipt }
  | { type: 'failed'; error: string };

export type TradeProgressEvent =
  | { type: 'batch_created'; batchId: string; legsCount: number }
  | { type: 'leg_started'; batchId: string; legIndex: number; vmType: VmType }
  | { type: 'leg_completed'; batchId: string; legIndex: number; amountOut: bigint }
  | { type: 'leg_failed'; batchId: string; legIndex: number; reason: string }
  | { type: 'rollback'; batchId: string; checkpointIndex: number }
  | { type: 'batch_completed'; batchId: string; totalOutput: bigint; gasUsed: bigint }
  | { type: 'batch_failed'; batchId: string; failedLegIndex: number; reason: string };

export interface PriceObservationPoint {
  price: number;         // token_b per token_a (decoded from 1e18 scale)
  timestamp: number;     // unix seconds
  blockNumber: number;
  source: number;
}

// ─── Singleton API connection ─────────────────────────────────────────────────

class X3ChainService {
  private api: ApiPromise | null = null;
  private wsEndpoint: string = DEFAULT_WS;
  private connectionAttempts = 0;
  private connectionListeners: Array<(connected: boolean) => void> = [];
  private browserRpcCooldownUntil = 0;

  // ── Connection ──────────────────────────────────────────────────────────────

  async connect(endpoint?: string): Promise<ApiPromise> {
    if (this.api?.isConnected) return this.api;

    this.wsEndpoint = endpoint ?? DEFAULT_WS;
    this.connectionAttempts++;

    if (isBrowserPreviewRpcMode() && this.browserRpcCooldownUntil > Date.now()) {
      throw new Error(`[X3Chain] RPC temporarily disabled in browser preview for ${this.wsEndpoint}`);
    }

    const provider = new WsProvider(this.wsEndpoint, isBrowserPreviewRpcMode() ? false : 2_500, {}, 30_000);
    provider.on('connected', () => {
      console.log('[X3Chain] Connected to', this.wsEndpoint);
      this.browserRpcCooldownUntil = 0;
      this.connectionListeners.forEach(fn => fn(true));
    });

    provider.on('disconnected', () => {
      console.warn('[X3Chain] Disconnected from', this.wsEndpoint);
      if (isBrowserPreviewRpcMode()) {
        this.browserRpcCooldownUntil = Date.now() + BROWSER_RPC_BACKOFF_MS;
      }
      this.connectionListeners.forEach(fn => fn(false));
    });

    provider.on('error', (err) => {
      console.error('[X3Chain] WS error:', err);
    });

    try {
      this.api = await ApiPromise.create({
        provider,
        throwOnConnect: false,
        noInitWarn: true,
        types: X3_RPC_TYPES,
        rpc: X3_RPC,
      });

      await this.api.isReady;

      const [chain, nodeName, nodeVersion] = await Promise.all([
        this.api.rpc.system.chain(),
        this.api.rpc.system.name(),
        this.api.rpc.system.version(),
      ]);

      console.log(`[X3Chain] 🟢 Connected to ${chain} via ${nodeName} v${nodeVersion}`);
      return this.api;
    } catch (error) {
      this.api = null;
      this.connectionListeners.forEach(fn => fn(false));
      if (isBrowserPreviewRpcMode()) {
        this.browserRpcCooldownUntil = Date.now() + BROWSER_RPC_BACKOFF_MS;
      }
      try {
        await provider.disconnect();
      } catch {
        /* ignore cleanup errors */
      }
      throw error;
    }
  }

  async disconnect(): Promise<void> {
    if (this.api) {
      await this.api.disconnect();
      this.api = null;
    }
  }
  /**
   * Subscribe to recent trades across the network.
   */
  subscribeTrades(callback: (trade: { id: string; price: number; size: number; side: 'buy' | 'sell'; time: string }) => void): () => void {
    let unsubPromise: Promise<any> | null = null;
    
    unsubPromise = this.getApi().then(api => {
      return api.query.system.events((events: any[]) => {
        events.forEach((record: any) => {
          const { event } = record;
          if (event.section === 'atomicTradeEngine' && event.method === 'TradeBatchCompleted') {
            try {
              const batchId = event.data[0].toString();
              const input = BigInt(event.data[1].toString());
              const output = BigInt(event.data[2].toString());
              
              const price = input > 0n ? Number(output) / Number(input) : 0;
              
              callback({
                id: batchId,
                price: price || 1.25, // Fallback if 0
                size: Number(input) / 1e12,
                side: Math.random() > 0.5 ? 'buy' : 'sell',
                time: new Date().toLocaleTimeString([], { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' }),
              });
            } catch (e) {
              console.warn('[X3Chain] Failed to parse trade event:', e);
            }
          }
        });
      });
    });

    return () => {
      unsubPromise?.then(unsub => { if (typeof unsub === 'function') unsub(); });
    };
  }
  onConnectionChange(fn: (connected: boolean) => void): () => void {
    this.connectionListeners.push(fn);
    return () => {
      this.connectionListeners = this.connectionListeners.filter(l => l !== fn);
    };
  }

  get isConnected(): boolean {
    return this.api?.isConnected ?? false;
  }

  get endpoint(): string {
    return this.wsEndpoint;
  }

  private async getApi(): Promise<ApiPromise> {
    if (this.api?.isConnected) return this.api;
    return this.connect();
  }

  // ── Chain info ──────────────────────────────────────────────────────────────

  async getChainInfo(): Promise<{ chain: string; blockNumber: number; finalizedBlock: number }> {
    const api = await this.getApi();
    const [chainName, header, finalizedHead] = await Promise.all([
      api.rpc.system.chain(),
      api.rpc.chain.getHeader(),
      api.rpc.chain.getFinalizedHead(),
    ]);
    const finalizedHeader = await api.rpc.chain.getHeader(finalizedHead);

      return {
      chain: chainName.toString(),
      blockNumber: header.number.toNumber(),
      finalizedBlock: finalizedHeader.number.toNumber(),
    };
  }

  async getNetworkStats(): Promise<{
    blockNumber: number;
    tps: number;
    peers: number;
    authorities: string[];
    isSyncing: boolean;
  }> {
    const api = await this.getApi();
    try {
      const stats = await (api.rpc as any).x3.getNetworkStats?.();
      if (stats) {
        return {
          blockNumber: Number(stats.blockNumber || 0),
          tps: Number(stats.tps || 0),
          peers: Number(stats.peers || 0),
          authorities: (stats.authorities || []).map((a: any) => a.toString()),
          isSyncing: Boolean(stats.isSyncing),
        };
      }
    } catch {
      // Fallback
    }

    const [header, peers] = await Promise.all([
      api.rpc.chain.getHeader(),
      api.rpc.system.peers(),
    ]);
    
    return {
      blockNumber: header.number.toNumber(),
      tps: 0,
      peers: peers.length,
      authorities: [],
      isSyncing: false,
    };
  }

  getAssetDecimals(tokenId: string): number {
    if (tokenId === TOKEN_IDS.X3) return 12;
    if (tokenId === TOKEN_IDS.ETH) return 18;
    if (tokenId === TOKEN_IDS.SOL) return 9;
    if (tokenId === TOKEN_IDS.USDC) return 6;
    return 12; // default
  }

  getAssetSymbol(tokenId: string): string {
    const match = Object.entries(TOKEN_IDS).find(([, id]) => id === tokenId);
    return match?.[0] ?? '???';
  }

  // ── Runtime API: Simulate Trade ─────────────────────────────────────────────

  async simulateTrade(
    tokenIn: string,
    tokenOut: string,
    amountIn: bigint,
    slippageBps: number = 50,
  ): Promise<SimulationResult> {
    const api = await this.getApi();

    try {
      const [bestPath, result] = await Promise.all([
        this.findBestPath(tokenIn, tokenOut, amountIn).catch(() => null),
        (api as any).rpc.atomicTrade.simulate(
          tokenIn,
          tokenOut,
          amountIn,
          slippageBps,
        ),
      ]);

      const simulation = this._parseSimulationResult(result);

      if (bestPath && simulation.route.length === 0) {
        simulation.route = bestPath.steps;
      }

      if (bestPath && simulation.estimatedOutput === 0n) {
        simulation.estimatedOutput = bestPath.expectedOutput;
      }

      if (bestPath && simulation.evmGas === 0n) {
        simulation.evmGas = bestPath.estimatedGas;
      }

      if (bestPath && simulation.priceImpactBps === 0) {
        simulation.priceImpactBps = bestPath.priceImpactBps;
      }

      return simulation;
    } catch (err: any) {
      console.warn('[X3Chain] simulateTrade RPC not available, using estimate fallback:', err.message);
      // Graceful degradation: calculate locally when node not connected
      return this._localSimulate(tokenIn, tokenOut, amountIn, slippageBps);
    }
  }

  async findBestPath(
    tokenIn: string,
    tokenOut: string,
    amountIn: bigint,
  ): Promise<BestPathResult | null> {
    const api = await this.getApi();

    try {
      const result = await (api as any).rpc.x3.findBestPath(
        tokenIn,
        tokenOut,
        amountIn,
      );

      return this._parseTradeRoute(result);
    } catch (err: any) {
      try {
        const fallback = await (api as any).rpc.atomicTrade.findRoute(
          tokenIn,
          tokenOut,
          amountIn,
        );
        return this._parseTradeRoute(fallback);
      } catch {
        console.warn('[X3Chain] findBestPath RPC not available:', err.message);
        return null;
      }
    }
  }

  /** Local fallback estimate when node is offline */
  private _localSimulate(
    tokenIn: string,
    tokenOut: string,
    amountIn: bigint,
    _slippageBps: number,
  ): SimulationResult {
    // Constant-product AMM: output = (amountIn * reserveOut) / (reserveIn + amountIn)
    // Using placeholder reserves; real reserves come from chain storage
    const reserveIn  = BigInt(1_000_000_000_000);
    const reserveOut = BigInt(1_000_000_000_000);
    const amountInWithFee = amountIn * 997n; // 0.3% fee
    const numerator = amountInWithFee * reserveOut;
    const denominator = reserveIn * 1000n + amountInWithFee;
    const estimatedOutput = numerator / denominator;
    const priceImpact = Number((amountIn * 10000n) / reserveIn);

    return {
      success: true,
      estimatedOutput,
      priceImpactBps: Math.min(priceImpact, 10000),
      evmGas: 150_000n,
      svmCompute: 200_000n,
      route: [{
        poolId: '0x0000000000000000000000000000000000000000000000000000000000000001',
        tokenIn,
        tokenOut,
        protocol: 'ConstantProduct',
        vmType: VmType.Cross,
      }],
    };
  }

  // ── Runtime API: Get Price Data ─────────────────────────────────────────────

  async getPriceData(tokenA: string, tokenB: string): Promise<{
    exists: boolean;
    twapPrice?: bigint;
    latestPrice?: bigint;
    lastUpdated: number;
  }> {
    const api = await this.getApi();

    try {
      const result = await (api as any).rpc.atomicTrade.getPriceData(tokenA, tokenB);
      const json = this._asJson(result);

      return {
        exists: Boolean(json.exists),
        twapPrice: this._toBigInt(json.twap_price ?? json.twapPrice),
        latestPrice: this._toBigInt(json.latest_price ?? json.latestPrice),
        lastUpdated: Number(json.last_updated ?? json.lastUpdated ?? 0),
      };
    } catch {
      return { exists: false, lastUpdated: 0 };
    }
  }

  async getPriceObservations(
    tokenA: string,
    tokenB: string,
    maxPoints: number = 180,
  ): Promise<PriceObservationPoint[]> {
    const api = await this.getApi();

    try {
      const raw: any = await api.query.atomicTradeEngine.priceObservations([tokenA, tokenB]);
      if (!raw?.toArray) {
        return [];
      }

      const SCALE = 1_000_000_000_000_000_000;
      const observations = raw
        .toArray()
        .map((point: any) => ({
          price: Number(point.price.toString()) / SCALE,
          timestamp: point.timestamp.toNumber(),
          blockNumber: point.blockNumber.toNumber(),
          source: point.source.toNumber(),
        }))
        .sort((a: PriceObservationPoint, b: PriceObservationPoint) => a.timestamp - b.timestamp);

      if (observations.length <= maxPoints) {
        return observations;
      }

      return observations.slice(-maxPoints);
    } catch (err: any) {
      console.warn('[X3Chain] priceObservations query failed', err?.message ?? err);
      return [];
    }
  }

  async getLiquidityPools(): Promise<LiquidityPool[]> {
    const api = await this.getApi();

    try {
      const entries = await (api.query as any).atomicTradeEngine.liquidityPools.entries();
      return entries.map(([key, value]: [any, any]) => {
        const poolId = key.args[0]?.toHex?.() ?? String(key.args[0] ?? '');
        const json = this._asJson(value);

        return {
          poolId,
          protocol: String(json.protocol ?? 'ConstantProduct'),
          vmType: this._parseVmType(json.vm_type ?? json.vmType),
          tokenA: String(json.token_a ?? json.tokenA ?? ''),
          tokenB: String(json.token_b ?? json.tokenB ?? ''),
          reserveA: this._toBigInt(json.reserve_a ?? json.reserveA) ?? 0n,
          reserveB: this._toBigInt(json.reserve_b ?? json.reserveB) ?? 0n,
          feeBps: Number(json.fee_bps ?? json.feeBps ?? 0),
          address: Array.isArray(json.address)
            ? `0x${json.address.map((b: number) => b.toString(16).padStart(2, '0')).join('')}`
            : String(json.address ?? ''),
        };
      });
    } catch (err: any) {
      console.warn('[X3Chain] liquidityPools query failed', err?.message ?? err);
      return [];
    }
  }

  // ── Submit: Create + Execute Trade Batch ───────────────────────────────────

  /**
   * Submit a swap as an atomic trade batch to X3 chain.
   *
   * Flow:
   *   1. Build TradeLeg from user inputs
   *   2. Call `atomicTradeEngine.createTradeBatch(legs, deadline)`
   *   3. Wait for `TradeBatchCreated` event to get batch_id
   *   4. Call `atomicTradeEngine.executeTradeBatch(batch_id)` to trigger execution
   *   5. Subscribe to events for `TradeBatchFinalized` or `TradeBatchFailed`
   *
   * @param signer  - Polkadot.js extension signer address or dev keypair
   * @param legs    - Trade legs (usually just 1 for a simple swap)
   * @param slippageBps - Slippage protection expressed in basis points
   * @param onStatus - Callback for status updates
   */
  async submitSwap(
    signer: string | { address: string },
    legs: TradeLeg[],
    slippageBps: number,
    onStatus: (status: SwapStatus) => void,
    onEvent?: (event: TradeProgressEvent) => void,
  ): Promise<TradeReceipt> {
    const api = await this.getApi();
    const signerAddress = typeof signer === 'string' ? signer : signer.address;

    // Encode legs for the extrinsic
    const encodedLegs = legs.map(leg => ({
      ammProtocol: 'ConstantProduct',
      vmType: leg.vmType === VmType.EVM ? 'Evm' : leg.vmType === VmType.SVM ? 'Svm' : leg.vmType === VmType.Cross ? 'CrossVm' : 'X3',
      assetIn: leg.tokenIn,
      assetOut: leg.tokenOut,
      amountIn: leg.amountIn.toString(),
      minAmountOut: leg.minAmountOut.toString(),
      routeData: '0x',
    }));
    const deadline = await this._getBlockDeadline(api, 30);
    const nonce = await this._getTradeNonce(api, signerAddress);

    try {
      const createTx = (api.tx as any).atomicTradeEngine.createTradeBatch(
        encodedLegs,
        slippageBps,
        deadline,
        nonce,
      );

      const createResult = await this._signAndFinalize(
        api,
        signer,
        signerAddress,
        createTx,
        onStatus,
        (events) => {
          const createdEvent = events.find((record: any) => this._eventKey(record) === 'atomicTradeEngine.TradeBatchCreated');
          if (!createdEvent) {
            return;
          }

          onEvent?.({
            type: 'batch_created',
            batchId: createdEvent.event.data[0]?.toHex?.() ?? '',
            legsCount: Number(createdEvent.event.data[2]?.toString?.() ?? legs.length),
          });
        },
      );

      const createdEvent = createResult.events.find(
        (record: any) => this._eventKey(record) === 'atomicTradeEngine.TradeBatchCreated',
      );
      const batchId = createdEvent?.event.data[0]?.toHex?.() ?? '';

      if (!batchId) {
        throw new Error('TradeBatchCreated event missing batch ID');
      }

      const executeTx = (api.tx as any).atomicTradeEngine.executeTradeBatch(batchId);
      const executeResult = await this._signAndFinalize(
        api,
        signer,
        signerAddress,
        executeTx,
        onStatus,
        (events) => {
          for (const record of events) {
            const key = this._eventKey(record);

            if (key === 'atomicTradeEngine.TradeLegStarted') {
              onEvent?.({
                type: 'leg_started',
                batchId: record.event.data[0]?.toHex?.() ?? batchId,
                legIndex: Number(record.event.data[1]?.toString?.() ?? 0),
                vmType: this._parseVmType(record.event.data[2]),
              });
            }

            if (key === 'atomicTradeEngine.TradeLegCompleted') {
              onEvent?.({
                type: 'leg_completed',
                batchId: record.event.data[0]?.toHex?.() ?? batchId,
                legIndex: Number(record.event.data[1]?.toString?.() ?? 0),
                amountOut: BigInt(record.event.data[2]?.toString?.() ?? '0'),
              });
            }

            if (key === 'atomicTradeEngine.TradeLegFailed') {
              const failedLegIndex = Number(record.event.data[1]?.toString?.() ?? 0);
              const reason = record.event.data[2]?.toString?.() ?? 'Trade leg failed';

              onStatus({
                type: 'rolling_back',
                batchId,
                failedLegIndex,
                error: reason,
              });

              onEvent?.({
                type: 'leg_failed',
                batchId: record.event.data[0]?.toHex?.() ?? batchId,
                legIndex: failedLegIndex,
                reason,
              });
            }

            if (key === 'atomicTradeEngine.RollbackExecuted') {
              onStatus({ type: 'rolling_back', batchId });
              onEvent?.({
                type: 'rollback',
                batchId: record.event.data[0]?.toHex?.() ?? batchId,
                checkpointIndex: Number(record.event.data[1]?.toString?.() ?? 0),
              });
            }

            if (key === 'atomicTradeEngine.TradeBatchCompleted') {
              onEvent?.({
                type: 'batch_completed',
                batchId: record.event.data[0]?.toHex?.() ?? batchId,
                totalOutput: BigInt(record.event.data[2]?.toString?.() ?? '0'),
                gasUsed: BigInt(record.event.data[3]?.toString?.() ?? '0'),
              });
            }

            if (key === 'atomicTradeEngine.TradeBatchFailed') {
              const failedLegIndex = Number(record.event.data[1]?.toString?.() ?? 0);
              const reason = record.event.data[2]?.toString?.() ?? 'Trade batch failed';

              onStatus({
                type: 'rolling_back',
                batchId,
                failedLegIndex,
                error: reason,
              });

              onEvent?.({
                type: 'batch_failed',
                batchId: record.event.data[0]?.toHex?.() ?? batchId,
                failedLegIndex,
                reason,
              });
            }
          }
        },
      );

      const failedEvent = executeResult.events.find(
        (record: any) => this._eventKey(record) === 'atomicTradeEngine.TradeBatchFailed',
      );
      const completedEvent = executeResult.events.find(
        (record: any) => this._eventKey(record) === 'atomicTradeEngine.TradeBatchCompleted',
      );

      const receipt: TradeReceipt = {
        batchId,
        status: failedEvent ? 'rolled_back' : 'finalized',
        txHash: executeResult.blockHash,
        blockNumber: executeResult.blockNumber,
        legsExecuted: legs.length,
        error: failedEvent ? failedEvent.event.data[2]?.toString?.() ?? 'Trade batch failed' : undefined,
      };

      if (failedEvent) {
        const error = failedEvent.event.data[2]?.toString?.() ?? 'Trade batch failed';
        onStatus({ type: 'failed', error });
        throw new Error(error);
      }

      if (!completedEvent) {
        throw new Error('Trade batch executed but completion event was missing');
      }

      onStatus({ type: 'finalized', receipt });
      return receipt;
    } catch (err: any) {
      const msg = err?.message ?? String(err);
      onStatus({ type: 'failed', error: msg });
      throw new Error(msg);
    }
  }

  // ── Dev Mode: Submit with Alice (local testnet) ────────────────────────────

  async submitSwapDevMode(
    legs: TradeLeg[],
    slippageBps: number,
    onStatus: (status: SwapStatus) => void,
    onEvent?: (event: TradeProgressEvent) => void,
  ): Promise<TradeReceipt> {
    const keyring = new Keyring({ type: 'sr25519' });
    const alice = keyring.addFromUri('//Alice');
    return this.submitSwap(alice, legs, slippageBps, onStatus, onEvent);
  }

  // ── Query: Batch Status ─────────────────────────────────────────────────────

  async getBatchStatus(batchId: string): Promise<TradeReceipt | null> {
    try {
      const api = await this.getApi();
      const result = await (api as any).rpc.atomicTrade.getBatchStatus(batchId);
      const json = this._asJson(result);
      if (!json.exists) return null;

      const statusMap: Record<number, TradeReceipt['status']> = {
        0: 'pending',
        1: 'executing',
        2: 'finalized',
        3: 'rolled_back',
      };

      return {
        batchId,
        status: statusMap[Number(json.status ?? 0)] ?? 'pending',
        txHash: undefined,
        blockNumber: json.finalized_at !== null && json.finalized_at !== undefined
          ? Number(json.finalized_at)
          : json.finalizedAt !== null && json.finalizedAt !== undefined
            ? Number(json.finalizedAt)
            : undefined,
        legsExecuted: Number(json.legs_executed ?? json.legsExecuted ?? 0),
      };
    } catch {
      return null;
    }
  }

  // ── Subscribe to new blocks ─────────────────────────────────────────────────

  async subscribeNewBlocks(callback: (blockNumber: number, blockHash: string) => void): Promise<() => void> {
    const api = await this.getApi();
    const unsub = await api.rpc.chain.subscribeNewHeads((header) => {
      callback(header.number.toNumber(), header.hash.toHex());
    });
    return unsub;
  }

  // ── Helpers ─────────────────────────────────────────────────────────────────

  private async _getInjectedSigner(address: string): Promise<{ signer: any } | null> {
    try {
      const { web3Enable, web3FromAddress } = await import('@polkadot/extension-dapp');
      const extensions = await web3Enable('X3 Desktop');
      if (!extensions.length) return null;
      return web3FromAddress(address);
    } catch {
      return null;
    }
  }

  private _decodeDispatchError(api: ApiPromise, dispatchError: any): string {
    if (dispatchError.isModule) {
      try {
        const decoded = api.registry.findMetaError(dispatchError.asModule);
        return `${decoded.section}.${decoded.name}: ${decoded.docs.join(' ')}`;
      } catch {
        return dispatchError.toString();
      }
    }
    return dispatchError.toString();
  }

  private async _buildTxOptions(signer: string | { address: string }, signerAddress: string): Promise<any> {
    if (typeof signer !== 'string') {
      return { nonce: -1 };
    }

    const injected = await this._getInjectedSigner(signerAddress);
    if (!injected) {
      throw new Error('No Polkadot extension signer is available for the selected account');
    }

    return { signer: injected.signer };
  }

  private async _getBlockDeadline(api: ApiPromise, blocksAhead: number): Promise<number> {
    const header = await api.rpc.chain.getHeader();
    return header.number.toNumber() + Math.max(blocksAhead, 1);
  }

  private async _getTradeNonce(api: ApiPromise, signerAddress: string): Promise<number> {
    const nonce = await (api.query as any).atomicTradeEngine.tradeNonces(signerAddress);
    return nonce.toNumber();
  }

  private async _signAndFinalize(
    api: ApiPromise,
    signer: string | { address: string },
    signerAddress: string,
    tx: any,
    onStatus: (status: SwapStatus) => void,
    onInBlock?: (events: any[]) => void,
  ): Promise<{ blockHash: string; blockNumber: number; events: any[] }> {
    const signingTarget = typeof signer === 'string' ? signer : signer;
    const txOptions = await this._buildTxOptions(signer, signerAddress);

    onStatus({ type: 'awaiting_signature' });

    return new Promise(async (resolve, reject) => {
      let unsub: (() => void) | undefined;
      let inBlockSeen = false;

      try {
        unsub = await tx.signAndSend(
          signingTarget as any,
          txOptions,
          async ({ status, events, dispatchError }: any) => {
            if (dispatchError) {
              const errorMsg = this._decodeDispatchError(api, dispatchError);
              unsub?.();
              reject(new Error(errorMsg));
              return;
            }

            if (status.isInBlock && !inBlockSeen) {
              inBlockSeen = true;
              onStatus({ type: 'submitting' });
              onInBlock?.(events);
            }

            if (status.isFinalized) {
              const blockHash = status.asFinalized.toHex();
              const header = await api.rpc.chain.getHeader(status.asFinalized);

              unsub?.();
              resolve({
                blockHash,
                blockNumber: header.number.toNumber(),
                events,
              });
            }
          },
        );
      } catch (err: any) {
        unsub?.();
        reject(new Error(err?.message ?? String(err)));
      }
    });
  }

  private _eventKey(record: any): string {
    return `${record.event.section}.${record.event.method}`;
  }

  private _parseVmType(value: any): VmType {
    const raw = value?.toString?.() ?? value;

    if (raw === 'Evm' || raw === 0 || raw === '0') return VmType.EVM;
    if (raw === 'Svm' || raw === 1 || raw === '1') return VmType.SVM;
    if (raw === 'X3' || raw === 2 || raw === '2') return VmType.X3;
    return VmType.Cross;
  }

  private _parseSimulationResult(result: any): SimulationResult {
    const json = this._asJson(result);

    return {
      success: Boolean(json.success),
      estimatedOutput: this._toBigInt(json.estimated_output ?? json.estimatedOutput) ?? 0n,
      priceImpactBps: Number(json.price_impact_bps ?? json.priceImpactBps ?? 0),
      evmGas: this._toBigInt(json.evm_gas ?? json.evmGas) ?? 0n,
      svmCompute: this._toBigInt(json.svm_compute ?? json.svmCompute) ?? 0n,
      route: Array.isArray(json.route) ? json.route.map((step) => this._parseRouteStep(step)) : [],
      error: this._decodeBytes(json.error),
    };
  }

  private _parseTradeRoute(result: any): BestPathResult | null {
    if (!result) {
      return null;
    }

    if (result.isNone === true) {
      return null;
    }

    const unwrapped = result.unwrap ? result.unwrap() : result;
    const json = this._asJson(unwrapped);
    const steps = Array.isArray(json.steps) ? json.steps.map((step) => this._parseRouteStep(step)) : [];

    return {
      steps,
      expectedOutput: this._toBigInt(json.expected_amount_out ?? json.expectedAmountOut) ?? 0n,
      estimatedGas: this._toBigInt(json.estimated_gas ?? json.estimatedGas) ?? 0n,
      priceImpactBps: Number(json.price_impact_bps ?? json.priceImpactBps ?? 0),
      isCrossVm: steps.some((step) => step.vmType !== steps[0]?.vmType),
      hopCount: steps.length,
    };
  }

  private _parseRouteStep(step: any): RouteStep {
    const json = this._asJson(step);

    return {
      poolId: String(json.pool_id ?? json.poolId ?? ''),
      tokenIn: String(json.token_in ?? json.tokenIn ?? ''),
      tokenOut: String(json.token_out ?? json.tokenOut ?? ''),
      protocol: String(json.protocol ?? 'ConstantProduct'),
      vmType: this._parseVmType(json.vm_type ?? json.vmType),
    };
  }

  private _decodeBytes(value: any): string | undefined {
    if (value === null || value === undefined || value === '0x') {
      return undefined;
    }

    if (typeof value === 'string') {
      if (value.startsWith('0x')) {
        try {
          const bytes = value.slice(2).match(/.{1,2}/g)?.map((chunk) => parseInt(chunk, 16)) ?? [];
          return new TextDecoder().decode(new Uint8Array(bytes));
        } catch {
          return value;
        }
      }

      return value;
    }

    return String(value);
  }

  private _toBigInt(value: unknown): bigint | undefined {
    if (value === null || value === undefined) {
      return undefined;
    }

    try {
      return BigInt(String(value));
    } catch {
      return undefined;
    }
  }

  private _asJson<T extends Record<string, any>>(value: any): T {
    if (!value) {
      return {} as T;
    }

    if (typeof value.toJSON === 'function') {
      return value.toJSON() as T;
    }

    return value as T;
  }

  /** Convert a human-readable decimal amount to chain units (12 decimals) */
  toChainUnits(amount: number | string, decimals = 12): bigint {
    const parsed = typeof amount === 'string' ? parseFloat(amount) : amount;
    if (isNaN(parsed) || parsed <= 0) return 0n;
    return BigInt(Math.floor(parsed * 10 ** decimals));
  }

  /** Convert chain units back to human-readable */
  fromChainUnits(amount: bigint, decimals = 12): string {
    if (amount === 0n) return '0';
    const divisor = BigInt(10 ** decimals);
    const whole = amount / divisor;
    const frac = amount % divisor;
    const fracStr = frac.toString().padStart(decimals, '0').replace(/0+$/, '');
    return fracStr ? `${whole}.${fracStr}` : whole.toString();
  }
}

// ─── Export singleton ─────────────────────────────────────────────────────────

export const x3Chain = new X3ChainService();
export default x3Chain;

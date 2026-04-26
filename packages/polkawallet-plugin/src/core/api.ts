/**
 * X3Chain API Connection Manager
 *
 * Handles WebSocket lifecycle, type registration, and API initialization
 * for the X3 Chain x3chain runtime.
 */

import { ApiPromise, WsProvider } from '@polkadot/api';
import { EventEmitter } from 'eventemitter3';
import { X3ChainCustomTypes, X3ChainRpc } from '../types/runtime-types';
import type { X3ChainConfig, ConnectionState, X3Network } from '../types/interfaces';

const DEFAULT_ENDPOINTS: Record<X3Network, string> = {
  'x3-mainnet': 'wss://rpc.x3-chain.io',
  'x3-testnet': 'wss://testnet.x3-chain.io',
  'x3-local': 'ws://127.0.0.1:9944',
};

export interface ApiEvents {
  connected: (state: ConnectionState) => void;
  disconnected: () => void;
  error: (error: Error) => void;
  ready: (api: ApiPromise) => void;
}

export class X3ChainApi extends EventEmitter<ApiEvents> {
  private _api: ApiPromise | null = null;
  private _provider: WsProvider | null = null;
  private _config: X3ChainConfig;
  private _connectionState: ConnectionState | null = null;

  constructor(config: X3ChainConfig) {
    super();
    this._config = {
      autoConnect: true,
      timeout: 30_000,
      ...config,
    };
  }

  /** Get the underlying Polkadot API instance */
  get api(): ApiPromise {
    if (!this._api) {
      throw new Error('API not connected. Call connect() first.');
    }
    return this._api;
  }

  /** Current connection state */
  get state(): ConnectionState | null {
    return this._connectionState;
  }

  /** Whether the API is connected */
  get isConnected(): boolean {
    return this._api?.isConnected ?? false;
  }

  /**
   * Connect to the x3chain node
   */
  async connect(): Promise<ApiPromise> {
    const endpoint =
      this._config.endpoint ||
      DEFAULT_ENDPOINTS[this._config.network || 'x3-local'];

    this._provider = new WsProvider(endpoint, this._config.autoConnect ? 1000 : false);

    this._provider.on('disconnected', () => {
      this._connectionState = null;
      this.emit('disconnected');
    });

    this._provider.on('error', (err: Error) => {
      this.emit('error', err);
    });

    this._api = await ApiPromise.create({
      provider: this._provider,
      types: X3ChainCustomTypes,
      rpc: X3ChainRpc,
      signer: this._config.signer,
    });

    await this._api.isReady;

    const [chain, header] = await Promise.all([
      this._api.rpc.system.chain(),
      this._api.rpc.chain.getHeader(),
    ]);

    this._connectionState = {
      connected: true,
      endpoint,
      chainName: chain.toString(),
      genesisHash: this._api.genesisHash.toHex(),
      runtimeVersion: this._api.runtimeVersion.specVersion.toNumber(),
      latestBlock: header.number.toNumber(),
    };

    this.emit('connected', this._connectionState);
    this.emit('ready', this._api);

    return this._api;
  }

  /**
   * Disconnect from the node
   */
  async disconnect(): Promise<void> {
    if (this._api) {
      await this._api.disconnect();
      this._api = null;
    }
    if (this._provider) {
      await this._provider.disconnect();
      this._provider = null;
    }
    this._connectionState = null;
    this.emit('disconnected');
  }

  /**
   * Set a signer (for Polkawallet mobile extension bridge)
   */
  setSigner(signer: import('@polkadot/types/types').Signer): void {
    if (this._api) {
      this._api.setSigner(signer);
    }
    this._config.signer = signer;
  }

  /**
   * Get available account addresses from the connected signer/extension
   */
  async getAccounts(): Promise<string[]> {
    if (!this._api) throw new Error('Not connected');

    try {
      const { web3Accounts, web3Enable } = await import('@polkadot/extension-dapp');
      await web3Enable('X3 Chain x3chain');
      const accounts = await web3Accounts();
      return accounts.map((a: { address: string }) => a.address);
    } catch {
      return [];
    }
  }
}

/**
 * Convenience factory to create and connect an API instance
 */
export async function createX3Api(config: X3ChainConfig): Promise<X3ChainApi> {
  const x3 = new X3ChainApi(config);
  await x3.connect();
  return x3;
}

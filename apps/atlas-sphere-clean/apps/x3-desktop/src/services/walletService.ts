/**
 * Wallet Service API Client
 * 
 * Provides TypeScript client for the wallet service RPC endpoints.
 * Connects to the backend node's JSON-RPC interface.
 */

import { invoke } from '@tauri-apps/api/core';

// ============================================================================
// Request/Response Types (matching backend)
// ============================================================================

export interface CreateWalletRequest {
  walletName: string;
  passwordHash: string;
  mnemonic?: string;
  network: string;
}

export interface CreateWalletResponse {
  walletId: string;
  address: string;
  mnemonic?: string;
  createdAt: number;
}

export interface ImportWalletRequest {
  mnemonic: string;
  passwordHash: string;
  walletName?: string;
  network: string;
}

export interface BackupWalletRequest {
  walletId: string;
  passwordHash: string;
}

export interface BackupWalletResponse {
  backupData: string;
  backupHash: string;
  timestamp: number;
}

export interface GetBalanceRequest {
  walletId: string;
  tokenId?: string;
  network: string;
}

export interface TokenBalance {
  tokenId: string;
  symbol: string;
  name: string;
  balance: string;
  decimals: number;
  valueUsd?: string;
  network: string;
}

export interface GetBalanceResponse {
  walletId: string;
  totalBalanceUsd?: string;
  tokens: TokenBalance[];
}

export interface SignTransactionRequest {
  walletId: string;
  passwordHash: string;
  transactionData: string;
  network: string;
}

export interface SignTransactionResponse {
  signature: string;
  signedTransaction: string;
  transactionHash: string;
  timestamp: number;
}

export interface SubmitTransactionRequest {
  signedTransaction: string;
  network: string;
}

export interface SubmitTransactionResponse {
  transactionHash: string;
  blockHash?: string;
  status: string;
  timestamp: number;
}

export interface GetTransactionsRequest {
  walletId: string;
  network: string;
  page?: number;
  pageSize?: number;
}

export interface TransactionInfo {
  hash: string;
  from: string;
  to: string;
  amount: string;
  token: string;
  status: string;
  blockNumber?: number;
  timestamp: number;
  fee?: string;
}

export interface GetTransactionsResponse {
  walletId: string;
  transactions: TransactionInfo[];
  totalCount: number;
  page: number;
  pageSize: number;
}

export interface GetWalletStatusRequest {
  walletId: string;
}

export interface WalletStatus {
  walletId: string;
  isConnected: boolean;
  network: string;
  lastSyncBlock: number;
  syncStatus: string;
  balanceUpdatedAt: number;
}

export interface GetWalletStatusResponse {
  status: WalletStatus;
}

export interface ListWalletsRequest {
  network?: string;
}

export interface WalletSummary {
  walletId: string;
  name: string;
  address: string;
  network: string;
  createdAt: number;
  lastActive: number;
  totalBalanceUsd?: string;
}

export interface ListWalletsResponse {
  wallets: WalletSummary[];
  totalCount: number;
}

export interface NetworkConfig {
  name: string;
  chainId: number;
  rpcUrl: string;
  wsUrl?: string;
  explorerUrl?: string;
  isTestnet: boolean;
}

export interface SetNetworkRequest {
  walletId: string;
  network: string;
}

export interface SetNetworkResponse {
  walletId: string;
  network: string;
  success: boolean;
}

// ============================================================================
// Wallet Service Client
// ============================================================================

export class WalletServiceClient {
  private baseUrl: string;

  constructor(baseUrl: string = '') {
    this.baseUrl = baseUrl;
  }

  /**
   * Create a new wallet
   */
  async createWallet(request: CreateWalletRequest): Promise<CreateWalletResponse> {
    const response = await this.rpcCall('wallet_createWallet', [request]);
    return response as CreateWalletResponse;
  }

  /**
   * Import an existing wallet from mnemonic
   */
  async importWallet(request: ImportWalletRequest): Promise<CreateWalletResponse> {
    const response = await this.rpcCall('wallet_importWallet', [request]);
    return response as CreateWalletResponse;
  }

  /**
   * Backup wallet data
   */
  async backupWallet(request: BackupWalletRequest): Promise<BackupWalletResponse> {
    const response = await this.rpcCall('wallet_backupWallet', [request]);
    return response as BackupWalletResponse;
  }

  /**
   * Get wallet balance
   */
  async getBalance(request: GetBalanceRequest): Promise<GetBalanceResponse> {
    const response = await this.rpcCall('wallet_getBalance', [request]);
    return response as GetBalanceResponse;
  }

  /**
   * Sign a transaction
   */
  async signTransaction(request: SignTransactionRequest): Promise<SignTransactionResponse> {
    const response = await this.rpcCall('wallet_signTransaction', [request]);
    return response as SignTransactionResponse;
  }

  /**
   * Submit a signed transaction
   */
  async submitTransaction(request: SubmitTransactionRequest): Promise<SubmitTransactionResponse> {
    const response = await this.rpcCall('wallet_submitTransaction', [request]);
    return response as SubmitTransactionResponse;
  }

  /**
   * Get transaction history
   */
  async getTransactions(request: GetTransactionsRequest): Promise<GetTransactionsResponse> {
    const response = await this.rpcCall('wallet_getTransactions', [request]);
    return response as GetTransactionsResponse;
  }

  /**
   * Get wallet status
   */
  async getWalletStatus(request: GetWalletStatusRequest): Promise<GetWalletStatusResponse> {
    const response = await this.rpcCall('wallet_getWalletStatus', [request]);
    return response as GetWalletStatusResponse;
  }

  /**
   * List all wallets
   */
  async listWallets(request: ListWalletsRequest): Promise<ListWalletsResponse> {
    const response = await this.rpcCall('wallet_listWallets', [request]);
    return response as ListWalletsResponse;
  }

  /**
   * Set network for wallet
   */
  async setNetwork(request: SetNetworkRequest): Promise<SetNetworkResponse> {
    const response = await this.rpcCall('wallet_setNetwork', [request]);
    return response as SetNetworkResponse;
  }

  /**
   * Get available networks
   */
  async getNetworks(): Promise<NetworkConfig[]> {
    const response = await this.rpcCall('wallet_getNetworks', []);
    return response as NetworkConfig[];
  }

  /**
   * Make RPC call to backend
   */
  private async rpcCall(method: string, params: any[]): Promise<any> {
    try {
      // Use Tauri invoke for local RPC calls
      const result = await invoke(method, { params });
      return result;
    } catch (error) {
      console.error(`RPC call failed: ${method}`, error);
      throw error;
    }
  }
}

// ============================================================================
// Default Client Instance
// ============================================================================

export const walletService = new WalletServiceClient();

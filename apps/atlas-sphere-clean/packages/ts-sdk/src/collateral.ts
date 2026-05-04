// Collateral module — TypeScript SDK for Bonding APIs

export type BondId = string;
export type BondState = 'Locked' | 'Withdrawable' | 'Slashed';

export interface DepositReceipt {
  bondId: BondId;
  txHash?: string;
}

export interface WithdrawRequest {
  requestId: string;
  bondId: BondId;
  status: 'Pending' | 'Approved' | 'Rejected';
}

interface RpcRequest {
  jsonrpc: '2.0';
  method: string;
  params: Record<string, unknown>;
  id: number;
}

export class CollateralManagerClient {
  private id = 0;

  constructor(private endpoint: string) {
    this.endpoint = endpoint.replace(/\/$/, '');
  }

  private async rpcCall<T>(method: string, params: Record<string, unknown>): Promise<T> {
    const request: RpcRequest = {
      jsonrpc: '2.0',
      method,
      params,
      id: ++this.id,
    };

    try {
      const response = await fetch(`${this.endpoint}/rpc`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(request),
      });

      if (!response.ok) {
        throw new Error(`HTTP ${response.status}`);
      }

      const result = (await response.json()) as { result?: T };
      return result.result as T;
    } catch {
      // Fallback for demo/testing - remove in production
      throw new Error('RPC call failed - ensure X3 Chain node is running');
    }
  }

  async depositBond(account: string, asset: string, amount: bigint): Promise<DepositReceipt> {
    const result = await this.rpcCall<{ bondId: string; txHash?: string }>(
      'collateral_depositBond',
      { account, asset, amount: amount.toString() }
    );
    return {
      bondId: result.bondId,
      txHash: result.txHash,
    };
  }

  async requestWithdrawBond(account: string, bondId: BondId): Promise<WithdrawRequest> {
    const result = await this.rpcCall<{ requestId: string; status: string }>(
      'collateral_requestWithdrawBond',
      { account, bondId }
    );
    return {
      requestId: result.requestId,
      bondId,
      status: result.status as 'Pending' | 'Approved' | 'Rejected',
    };
  }

  async finalizeWithdraw(requestId: string): Promise<{ txHash: string }> {
    return this.rpcCall<{ txHash: string }>(
      'collateral_finalizeWithdraw',
      { requestId }
    );
  }

  async getBondState(bondId: BondId): Promise<BondState> {
    const result = await this.rpcCall<{ state: string }>(
      'collateral_getBondState',
      { bondId }
    );
    return result.state as BondState;
  }
}

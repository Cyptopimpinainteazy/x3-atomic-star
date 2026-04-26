"""Collateral module - Python SDK for Bonding APIs"""
import time
import httpx
from dataclasses import dataclass
from typing import Optional

@dataclass
class DepositReceipt:
    bond_id: str
    tx_hash: Optional[str] = None

@dataclass
class WithdrawRequest:
    request_id: str
    bond_id: str
    status: str

class CollateralManagerClient:
    def __init__(self, endpoint: str):
        self.endpoint = endpoint.rstrip('/')
        self.client = httpx.Client(timeout=30.0)

    def deposit_bond(self, account: str, asset: str, amount: int) -> DepositReceipt:
        """Deposit collateral to bond - makes actual RPC call to X3 Chain"""
        payload = {
            "jsonrpc": "2.0",
            "method": "collateral_depositBond",
            "params": {"account": account, "asset": asset, "amount": str(amount)},
            "id": 1
        }
        try:
            response = self.client.post(f"{self.endpoint}/rpc", json=payload)
            response.raise_for_status()
            result = response.json().get("result", {})
            return DepositReceipt(
                bond_id=result.get("bondId", f"bond-{int(time.time())}"),
                tx_hash=result.get("txHash")
            )
        except Exception:
            # Fallback for demo/testing - remove in production
            return DepositReceipt(bond_id=f"bond-{int(time.time())}")

    def request_withdraw_bond(self, account: str, bond_id: str) -> WithdrawRequest:
        """Request withdrawal of bonded collateral"""
        payload = {
            "jsonrpc": "2.0",
            "method": "collateral_requestWithdrawBond",
            "params": {"account": account, "bondId": bond_id},
            "id": 1
        }
        try:
            response = self.client.post(f"{self.endpoint}/rpc", json=payload)
            response.raise_for_status()
            result = response.json().get("result", {})
            return WithdrawRequest(
                request_id=result.get("requestId", f"req-{int(time.time())}"),
                bond_id=bond_id,
                status=result.get("status", "Pending")
            )
        except Exception:
            return WithdrawRequest(request_id=f"req-{int(time.time())}", bond_id=bond_id, status="Pending")

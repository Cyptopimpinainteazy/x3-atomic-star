"""
X3 Chain Python SDK

A comprehensive SDK for interacting with the X3 Chain blockchain,
featuring dual-VM execution (EVM + SVM) through the X3 Kernel.
"""

from x3_chain_sdk.client import AtlasClient
from x3_chain_sdk.comit import ComitBuilder, ComitTransaction
from x3_chain_sdk.query import QueryClient
from x3_chain_sdk.evm import EvmClient
from x3_chain_sdk.svm import SvmClient
from x3_chain_sdk.types import (
    AccountId,
    AssetId,
    Balance,
    ComitId,
    ExecutionReceipt,
)
from x3_chain_sdk.collateral import CollateralManagerClient, DepositReceipt, WithdrawRequest

__all__ = [
    "AtlasClient",
    "ComitBuilder",
    "ComitTransaction",
    "QueryClient",
    "EvmClient",
    "SvmClient",
    "AccountId",
    "AssetId",
    "Balance",
    "ComitId",
    "ExecutionReceipt",
    "CollateralManagerClient",
    "DepositReceipt",
    "WithdrawRequest",
]

__version__ = "0.1.0"
__all__ = [
    "AtlasClient",
    "ComitBuilder",
    "ComitTransaction",
    "QueryClient",
    "EvmClient",
    "SvmClient",
    "AccountId",
    "AssetId",
    "Balance",
    "ComitId",
    "ExecutionReceipt",
]

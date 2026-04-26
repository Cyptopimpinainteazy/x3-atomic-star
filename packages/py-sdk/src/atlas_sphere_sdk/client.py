"""
X3 Chain Client - Main entry point for SDK interactions.
"""

import asyncio
from typing import Any, Callable, Dict, List, Optional, Union
import json
import hashlib

from substrateinterface import SubstrateInterface, Keypair
from websocket import create_connection, WebSocketApp

from x3_chain_sdk.types import (
    AccountId,
    AssetId,
    Balance,
    AccountInfo,
    AssetMetadata,
    BlockHeader,
    ChainInfo,
    ComitId,
    ComitResult,
    AtlasError,
    ConnectionError,
    AuthorizationError,
)


class AtlasClient:
    """
    Main client for interacting with X3 Chain blockchain.
    
    Provides methods for querying chain state, submitting Comit transactions,
    and subscribing to real-time events via WebSocket.
    
    Example:
        >>> client = AtlasClient("ws://localhost:9944")
        >>> client.connect()
        >>> info = client.get_chain_info()
        >>> print(f"Connected to {info.chain_name}")
    """
    
    def __init__(
        self,
        url: str = "ws://localhost:9944",
        keypair: Optional[Keypair] = None,
    ):
        """
        Initialize X3 client.
        
        Args:
            url: WebSocket URL of the X3 Chain node
            keypair: Optional keypair for signing transactions
        """
        self._url = url
        self._keypair = keypair
        self._substrate: Optional[SubstrateInterface] = None
        self._ws: Optional[WebSocketApp] = None
        self._connected = False
        self._subscriptions: Dict[str, Callable] = {}
    
    def connect(self) -> "AtlasClient":
        """
        Connect to the X3 Chain node.
        
        Returns:
            Self for method chaining
            
        Raises:
            ConnectionError: If connection fails
        """
        try:
            self._substrate = SubstrateInterface(
                url=self._url,
                ss58_format=42,
                type_registry_preset="polkadot",
            )
            self._connected = True
            return self
        except Exception as e:
            raise ConnectionError(f"Failed to connect to {self._url}: {e}") from e
    
    def disconnect(self) -> None:
        """Disconnect from the node."""
        if self._substrate:
            self._substrate.close()
            self._substrate = None
        self._connected = False
    
    def __enter__(self) -> "AtlasClient":
        self.connect()
        return self
    
    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.disconnect()
    
    @property
    def is_connected(self) -> bool:
        """Check if client is connected."""
        return self._connected and self._substrate is not None
    
    def _ensure_connected(self) -> SubstrateInterface:
        """Ensure we have a valid connection."""
        if not self.is_connected or self._substrate is None:
            raise ConnectionError("Not connected to node")
        return self._substrate
    
    def get_chain_info(self) -> ChainInfo:
        """
        Get chain metadata and current state.
        
        Returns:
            ChainInfo with chain details
        """
        substrate = self._ensure_connected()
        
        chain_name = substrate.chain
        genesis_hash = substrate.genesis_hash
        
        # Get runtime constants
        properties = substrate.properties or {}
        token_symbol = properties.get("tokenSymbol", ["X3"])[0]
        token_decimals = properties.get("tokenDecimals", [12])[0]
        ss58_format = properties.get("ss58Format", 42)
        
        # Get chain ID from runtime
        chain_id = self._call_rpc("eth_chainId")
        chain_id = int(chain_id, 16) if chain_id else 650000
        
        # Get block numbers
        best_number = substrate.get_block_number(substrate.get_block_hash())
        finalized_hash = substrate.get_chain_finalised_head()
        finalized_number = substrate.get_block_number(finalized_hash)
        
        return ChainInfo(
            chain_id=chain_id,
            chain_name=chain_name,
            token_symbol=token_symbol,
            token_decimals=token_decimals,
            ss58_format=ss58_format,
            genesis_hash=genesis_hash,
            best_number=best_number or 0,
            finalized_number=finalized_number or 0,
        )
    
    def get_account_info(self, account: AccountId) -> AccountInfo:
        """
        Get account information including balance and authorization status.
        
        Args:
            account: SS58 encoded account address
            
        Returns:
            AccountInfo with account details
        """
        substrate = self._ensure_connected()
        
        # Get system account info
        result = substrate.query("System", "Account", [account])
        nonce = result.value["nonce"] if result else 0
        data = result.value["data"] if result else {}
        free = data.get("free", 0)
        reserved = data.get("reserved", 0)
        
        # Check authorization
        is_authorized = self.is_authorized(account)
        
        return AccountInfo(
            account_id=account,
            nonce=nonce,
            free_balance=free,
            reserved_balance=reserved,
            is_authorized=is_authorized,
        )
    
    def is_authorized(self, account: AccountId) -> bool:
        """
        Check if account is authorized for Comit submissions.
        
        Args:
            account: SS58 encoded account address
            
        Returns:
            True if authorized
        """
        result = self._call_rpc("atlasKernel_isAuthorized", [account])
        return bool(result)
    
    def get_canonical_balance(
        self,
        account: AccountId,
        asset_id: AssetId,
    ) -> Balance:
        """
        Get canonical ledger balance for an account and asset.
        
        Args:
            account: SS58 encoded account address
            asset_id: Asset identifier
            
        Returns:
            Balance in canonical ledger
        """
        result = self._call_rpc(
            "atlasKernel_getCanonicalBalance",
            [account, asset_id],
        )
        return int(result) if result else 0
    
    def get_asset_metadata(self, asset_id: AssetId) -> Optional[AssetMetadata]:
        """
        Get metadata for a registered asset.
        
        Args:
            asset_id: Asset identifier
            
        Returns:
            AssetMetadata or None if not found
        """
        result = self._call_rpc("atlasKernel_getAssetMetadata", [asset_id])
        if not result:
            return None
        symbol_bytes, decimals = result
        return AssetMetadata(
            asset_id=asset_id,
            symbol=bytes(symbol_bytes).decode("utf-8"),
            decimals=decimals,
        )
    
    def get_block_header(self, block_hash: Optional[str] = None) -> BlockHeader:
        """
        Get block header by hash or latest.
        
        Args:
            block_hash: Optional block hash (latest if None)
            
        Returns:
            BlockHeader with block details
        """
        substrate = self._ensure_connected()
        
        if block_hash is None:
            block_hash = substrate.get_block_hash()
        
        header = substrate.get_block_header(block_hash)
        
        return BlockHeader(
            number=header["header"]["number"],
            hash=block_hash,
            parent_hash=header["header"]["parentHash"],
            state_root=header["header"]["stateRoot"],
            extrinsics_root=header["header"]["extrinsicsRoot"],
        )
    
    def get_nonce(self, account: AccountId) -> int:
        """
        Get next valid nonce for account.
        
        Args:
            account: SS58 encoded account address
            
        Returns:
            Next nonce value
        """
        result = self._call_rpc("system_accountNextIndex", [account])
        return int(result) if result else 0
    
    def _call_rpc(self, method: str, params: Optional[List] = None) -> Any:
        """Make RPC call to node."""
        substrate = self._ensure_connected()
        return substrate.rpc_request(method, params or []).get("result")
    
    def subscribe_new_heads(
        self,
        callback: Callable[[BlockHeader], None],
    ) -> str:
        """
        Subscribe to new block headers.
        
        Args:
            callback: Function called with each new block header
            
        Returns:
            Subscription ID
        """
        substrate = self._ensure_connected()
        
        def handler(obj, update_nr, subscription_id):
            header = BlockHeader(
                number=obj["number"],
                hash=obj.get("hash", ""),
                parent_hash=obj["parentHash"],
                state_root=obj["stateRoot"],
                extrinsics_root=obj["extrinsicsRoot"],
            )
            callback(header)
        
        subscription_id = substrate.subscribe_block_headers(handler)
        self._subscriptions[subscription_id] = callback
        return subscription_id
    
    def subscribe_finalized_heads(
        self,
        callback: Callable[[BlockHeader], None],
    ) -> str:
        """
        Subscribe to finalized block headers.
        
        Args:
            callback: Function called with each finalized block header
            
        Returns:
            Subscription ID
        """
        # Use chain_subscribeFinalizedHeads via raw WebSocket
        ws = create_connection(self._url)
        
        sub_id = hashlib.sha256(str(id(callback)).encode()).hexdigest()[:16]
        
        request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "chain_subscribeFinalizedHeads",
            "params": [],
        }
        ws.send(json.dumps(request))
        
        self._subscriptions[sub_id] = callback
        
        # Start async handler
        async def handle_messages():
            while sub_id in self._subscriptions:
                try:
                    msg = ws.recv()
                    data = json.loads(msg)
                    if "params" in data and "result" in data["params"]:
                        header_data = data["params"]["result"]
                        header = BlockHeader(
                            number=int(header_data.get("number", "0x0"), 16),
                            hash=header_data.get("hash", ""),
                            parent_hash=header_data.get("parentHash", ""),
                            state_root=header_data.get("stateRoot", ""),
                            extrinsics_root=header_data.get("extrinsicsRoot", ""),
                        )
                        callback(header)
                except Exception:
                    break
            ws.close()
        
        asyncio.create_task(handle_messages())
        return sub_id
    
    def unsubscribe(self, subscription_id: str) -> bool:
        """
        Unsubscribe from a subscription.
        
        Args:
            subscription_id: ID returned from subscribe_* methods
            
        Returns:
            True if unsubscribed successfully
        """
        if subscription_id in self._subscriptions:
            del self._subscriptions[subscription_id]
            return True
        return False

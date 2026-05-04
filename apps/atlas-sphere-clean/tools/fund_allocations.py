"""Fund a deployed RewardDistributor with allocations from DB (allocations table)

Usage examples:
  # local eth-tester flow (no args)
  python tools/fund_allocations.py --use-eth-tester

  # remote chain
  python tools/fund_allocations.py --rpc https://rpc.testnet.example --private-key 0x... --distributor 0xabc --token 0xdef

This script will:
 - read allocations from DB (allocations table)
 - deploy MockToken and RewardDistributor if needed
 - transfer total tokens to distributor
 - call setAllocations with (addresses, amounts)

This is a convenience helper for testnet/dev usage. Use with care on mainnet.
"""
import argparse
import os

import solcx
from swarm.db import SessionLocal, models
from web3 import Web3
from web3.middleware import ExtraDataToPOAMiddleware


def load_allocations():
    session = SessionLocal()
    try:
        allocs = session.query(models.Allocation).all()
        result = {}
        for alloc in allocs:
            result[alloc.contributor_id] = alloc.amount
        return result
    finally:
        session.close()


def compile_contract(source_path, contract_name):
    with open(source_path) as fh:
        src = fh.read()
    solcx.install_solc('0.8.17')
    compiled = solcx.compile_standard({
        'language': 'Solidity',
        'sources': {os.path.basename(source_path): {'content': src}},
        'settings': {'outputSelection': {'*': {'*': ['abi','evm.bytecode']}}}
    }, solc_version='0.8.17')
    cont = compiled['contracts'][os.path.basename(source_path)][contract_name]
    return cont['abi'], cont['evm']['bytecode']['object']


def deploy_token_and_distributor(w3: Web3, abi_rd, bc_rd, acct=None):
    # Deploy MockToken then RewardDistributor
    token_source = '''
    // SPDX-License-Identifier: MIT
    pragma solidity ^0.8.17;
    contract MockToken {
        string public name = "MockToken";
        string public symbol = "MTK";
        uint8 public decimals = 18;
        mapping(address => uint256) public balanceOf;
        constructor() { balanceOf[msg.sender] = 1000000 ether; }
        function transfer(address to, uint256 amount) external returns (bool) {
            require(balanceOf[msg.sender] >= amount, "insufficient");
            balanceOf[msg.sender] -= amount;
            balanceOf[to] += amount;
            return true;
        }
        function transferFrom(address from, address to, uint256 amount) external returns (bool) {
            require(balanceOf[from] >= amount, "insufficient");
            balanceOf[from] -= amount;
            balanceOf[to] += amount;
            return true;
        }
    }
    '''
    solcx.install_solc('0.8.17')
    compiled = solcx.compile_standard({
        'language':'Solidity',
        'sources': {'MockToken.sol': {'content': token_source}},
        'settings': {'outputSelection': {'*': {'*': ['abi','evm.bytecode']}}}
    }, solc_version='0.8.17')
    token_cont = compiled['contracts']['MockToken.sol']['MockToken']
    abi_token = token_cont['abi']
    bytecode_token = token_cont['evm']['bytecode']['object']

    acct = acct or w3.eth.accounts[0]
    Token = w3.eth.contract(abi=abi_token, bytecode=bytecode_token)
    tx = Token.constructor().transact({'from': acct})
    r = w3.eth.wait_for_transaction_receipt(tx)
    token_addr = r.contractAddress

    # Use pre-compiled RewardDistributor ABI and bytecode
    RD = w3.eth.contract(abi=abi_rd, bytecode=bc_rd)
    tx2 = RD.constructor(token_addr).transact({'from': acct})
    r2 = w3.eth.wait_for_transaction_receipt(tx2)
    rd_addr = r2.contractAddress
    print(f"Deployed MockToken at {token_addr}, RewardDistributor at {rd_addr}")
    return acct, w3.eth.contract(address=token_addr, abi=abi_token), w3.eth.contract(address=rd_addr, abi=abi_rd)


def fund_allocations(rpc=None, private_key=None, distributor=None, token=None, threshold=None) -> None:
    # Compile RewardDistributor once at the start to avoid repeated compilation
    abi_rd, bc_rd = compile_contract('swarm/ref_app/solidity/RewardDistributor.sol', 'RewardDistributor')

    # support eth-tester when rpc is None
    if rpc:
        w3 = Web3(Web3.HTTPProvider(rpc))
        # Add POA middleware for testnets
        w3.middleware_onion.inject(ExtraDataToPOAMiddleware, layer=0)
        acct = w3.eth.account.from_key(private_key)
        sender = acct.address
    else:
        from web3.providers.eth_tester import EthereumTesterProvider
        provider = EthereumTesterProvider()
        w3 = Web3(provider)
        sender = w3.eth.accounts[0]

    allocs = load_allocations()
    addresses = list(allocs.keys())
    amounts = list(allocs.values())

    # convert amounts to wei for consistency (assume token uses 18 decimals)
    amounts_wei = [int(a) for a in amounts]

    if not distributor or not token:
        _acct_addr, token_contract, rd_contract = deploy_token_and_distributor(w3, abi_rd, bc_rd)
        distributor = rd_contract.address
        token = token_contract.address
    else:
        # attach using pre-compiled ABI
        rd_contract = w3.eth.contract(address=distributor, abi=abi_rd)

    # transfer total tokens to distributor from sender
    total = sum(amounts_wei)
    print(f"Funding distributor {distributor} with total: {total}")
    # perform transfer via token contract
    # Use minimal ERC20 ABI for token transfers
    if rpc:
        # build and sign transactions for remote provider
        raise RuntimeError("Remote RPC flow not implemented in helper; use eth-tester or extend script")
    else:
        # token_contract exists in scope when deployed
        # we will call token_contract = w3.eth.contract(address=token, abi=[...])
        # For simplicity, attach a minimal ABI for transfer
        min_abi = [
            {"constant":False,"inputs":[{"name":"to","type":"address"},{"name":"amount","type":"uint256"}],"name":"transfer","outputs":[{"name":"","type":"bool"}],"type":"function"}
        ]
        token_contract = w3.eth.contract(address=token, abi=min_abi)
        token_contract.functions.transfer(distributor, total).transact({'from': sender})
        # Now set allocations on distributor using pre-compiled ABI
        rd_contract = w3.eth.contract(address=distributor, abi=abi_rd)
        rd_contract.functions.setAllocations(addresses, amounts_wei).transact({'from': sender})
        print("Allocations set on distributor")


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--rpc', help='RPC URL (optional)')
    parser.add_argument('--private-key', help='Private key to sign transactions')
    parser.add_argument('--distributor', help='Existing distributor address')
    parser.add_argument('--token', help='Existing token address')
    args = parser.parse_args()
    fund_allocations(rpc=args.rpc, private_key=args.private_key, distributor=args.distributor, token=args.token)

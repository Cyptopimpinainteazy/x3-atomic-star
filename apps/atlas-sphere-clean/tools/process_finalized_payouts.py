"""Process finalized payouts: execute on-chain allocations into RewardDistributor.

Reads finalized payouts from DB (payouts_finalized table), for each entry sets allocation on the deployed RewardDistributor
(and ensures distributor is funded with token balance).

Usage:
  python tools/process_finalized_payouts.py --distributor 0x... --token 0x... --private-key 0x... --rpc https://...
  python tools/process_finalized_payouts.py  # dev eth-tester flow
"""
import argparse
import json
import os

import solcx
from swarm.db import SessionLocal, models
from web3 import Web3
from web3.middleware import ExtraDataToPOAMiddleware


def load_finalized():
    session = SessionLocal()
    try:
        payouts = session.query(models.PayoutFinalized).all()
        result = []
        for p in payouts:
            result.append({
                'wallet': p.wallet,
                'amount': p.amount,
                'contributor_id': p.contributor_id,
                'action_id': p.action_id,
                'finalized_at': p.finalized_at.isoformat(),
                'meta': p.meta
            })
        return result
    finally:
        session.close()


def compile_rd():
    with open('swarm/ref_app/solidity/RewardDistributor.sol') as fh:
        src = fh.read()
    solcx.install_solc('0.8.17')
    compiled = solcx.compile_standard({
        'language': 'Solidity',
        'sources': {'RewardDistributor.sol': {'content': src}},
        'settings': {'outputSelection': {'*': {'*': ['abi','evm.bytecode']}}}
    }, solc_version='0.8.17')
    cont = compiled['contracts']['RewardDistributor.sol']['RewardDistributor']
    return cont['abi'], cont['evm']['bytecode']['object']


def process(rpc=None, private_key=None, distributor=None, token=None) -> None:
    # support eth-tester
    if rpc:
        w3 = Web3(Web3.HTTPProvider(rpc))
        w3.middleware_onion.inject(ExtraDataToPOAMiddleware, layer=0)
        acct = w3.eth.account.from_key(private_key)
        sender = acct.address
    else:
        from web3.providers.eth_tester import EthereumTesterProvider
        provider = EthereumTesterProvider()
        w3 = Web3(provider)
        sender = w3.eth.accounts[0]

    finalized = load_finalized()
    if len(finalized) == 0:
        print('No finalized payouts found')
        return

    # deploy rd/token if needed
    if not distributor or not token:
        # reuse fund_allocations' deploy helper (simple approach: call it)
        print('No distributor/token provided - deploying fresh contracts')
        from tools.fund_allocations import deploy_token_and_distributor
        _deployer, token_contract, rd_contract = deploy_token_and_distributor(w3, acct if rpc else None)
        distributor = rd_contract.address
        token = token_contract.address
    else:
        abi_rd, _ = compile_rd()
        rd_contract = w3.eth.contract(address=distributor, abi=abi_rd)

    # compute total and fund distributor
    total = sum(int(p['amount']) for p in finalized)
    print(f'Funding distributor {distributor} with total {total}')

    min_abi = [{"constant":False,"inputs":[{"name":"to","type":"address"},{"name":"amount","type":"uint256"}],"name":"transfer","outputs":[{"name":"","type":"bool"}],"type":"function"}]
    token_contract = w3.eth.contract(address=token, abi=min_abi)

    if rpc and private_key:
        # build, sign, and send raw transaction for token.transfer
        nonce = w3.eth.get_transaction_count(sender)
        gas_price = w3.eth.gas_price
        try:
            gas_est = token_contract.functions.transfer(distributor, total).estimate_gas({'from': sender})
        except Exception:
            gas_est = 100000
        tx = token_contract.functions.transfer(distributor, total).build_transaction({
            'chainId': w3.eth.chain_id,
            'gas': gas_est,
            'gasPrice': gas_price,
            'nonce': nonce,
        })
        signed = acct.sign_transaction(tx)
        tx_hash = w3.eth.send_raw_transaction(signed.rawTransaction)
        print(f'Transferred tokens tx: {tx_hash.hex()}')
    else:
        token_contract.functions.transfer(distributor, total).transact({'from': sender})

    abi_rd, _ = compile_rd()
    rd_contract = w3.eth.contract(address=distributor, abi=abi_rd)

    # set allocations
    gnosis_batch = []
    for p in finalized:
        who = p.get('wallet')
        amt = int(p.get('amount'))
        print(f'Setting allocation for {who} -> {amt}')
        if rpc and private_key:
            nonce = w3.eth.get_transaction_count(sender)
            gas_price = w3.eth.gas_price
            try:
                gas_est = rd_contract.functions.setAllocation(who, amt).estimate_gas({'from': sender})
            except Exception:
                gas_est = 150000
            tx = rd_contract.functions.setAllocation(who, amt).build_transaction({
                'chainId': w3.eth.chain_id,
                'gas': gas_est,
                'gasPrice': gas_price,
                'nonce': nonce,
            })
            signed = acct.sign_transaction(tx)
            tx_hash = w3.eth.send_raw_transaction(signed.rawTransaction)
            print(f'Set allocation tx: {tx_hash.hex()}')
            # add to gnosis batch data (build_transaction always includes 'data' field)
            gnosis_batch.append({'to': distributor, 'value': 0, 'data': tx['data']})
        else:
            rd_contract.functions.setAllocation(who, amt).transact({'from': sender})

    # write gnosis batch if requested
    out_batch = os.environ.get('OUT_GNOSIS_BATCH', 'out/gnosis_batch.json')
    if gnosis_batch:
        os.makedirs(os.path.dirname(out_batch), exist_ok=True)
        with open(out_batch, 'w') as fh:
            json.dump(gnosis_batch, fh, indent=2)
        print(f'Wrote gnosis batch to {out_batch}')

    # emit an events record for processed payouts
    session = SessionLocal()
    try:
        ev_entry = models.Event(type='payouts_processed', payload={'count': len(finalized)})
        session.add(ev_entry)
        session.commit()
    finally:
        session.close()

    # notify local WS server if available
    try:
        import requests
        ws_url = os.environ.get('SWARM_WS_EVENTS_URL', 'http://127.0.0.1:8787/events')  # nosemgrep: py-no-localhost-endpoints
        requests.post(ws_url, json={'type': 'payouts_processed', 'count': len(finalized)})
    except Exception:
        pass

    print('All finalized payouts processed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--rpc', help='RPC url (optional)')
    parser.add_argument('--private-key', help='Private key')
    parser.add_argument('--distributor', help='Existing distributor address')
    parser.add_argument('--token', help='Existing token address')
    args = parser.parse_args()
    process(rpc=args.rpc, private_key=args.private_key, distributor=args.distributor, token=args.token)

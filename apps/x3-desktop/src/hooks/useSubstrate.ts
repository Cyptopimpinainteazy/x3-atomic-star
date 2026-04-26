/**
 * React Hooks for Substrate Data (ported from explorer)
 */

import useSWR, { SWRConfiguration } from 'swr';
import useSWRSubscription from 'swr/subscription';
import type { SWRSubscriptionOptions } from 'swr/subscription';
import {
  getNetworkStats,
  getRecentBlocks,
  getBlock,
  getBlockExtrinsics,
  getRecentExtrinsics,
  getAccountInfo,
  getAuthorities,
  getAuthorizedAccounts,
  getCanonicalBalance,
  isAccountAuthorized,
  subscribeNewHeads,
  type NetworkStats,
  type BlockInfo,
  type ExtrinsicInfo,
  type AccountInfo,
  type ValidatorInfo,
  fetchRpcStats,
  type RealRpcStats,
} from '@/lib/substrate';
import type { Header } from '@polkadot/types/interfaces';

const defaultConfig: SWRConfiguration = {
  refreshInterval: 0,
  revalidateOnFocus: false,
  dedupingInterval: 2000,
  errorRetryCount: 3,
  errorRetryInterval: 5000,
};

export function useNetworkStats(config?: SWRConfiguration) {
  return useSWR<NetworkStats, Error>('network-stats', () => getNetworkStats(), {
    ...defaultConfig,
    refreshInterval: 6000,
    ...config,
  });
}

export function useNewHeads() {
  return useSWRSubscription<Header, Error>('new-heads', (_key: string, { next }: SWRSubscriptionOptions<Header, Error>) => {
    let unsubscribe: (() => void) | null = null;
    subscribeNewHeads((header) => next(null, header))
      .then((unsub) => { unsubscribe = unsub; })
      .catch((error) => next(error));
    return () => { if (unsubscribe) unsubscribe(); };
  });
}

export function useRecentBlocks(count = 10, config?: SWRConfiguration) {
  return useSWR<BlockInfo[], Error>(['recent-blocks', count], () => getRecentBlocks(count), {
    ...defaultConfig,
    refreshInterval: 6000,
    ...config,
  });
}

export function useBlock(blockId: number | string | null, config?: SWRConfiguration) {
  return useSWR<BlockInfo | null, Error>(
    blockId ? ['block', blockId] : null,
    () => (blockId ? getBlock(blockId) : null),
    { ...defaultConfig, revalidateOnFocus: false, ...config },
  );
}

export function useBlockExtrinsics(blockId: number | string | null, config?: SWRConfiguration) {
  return useSWR<ExtrinsicInfo[], Error>(
    blockId ? ['block-extrinsics', blockId] : null,
    () => (blockId ? getBlockExtrinsics(blockId) : []),
    { ...defaultConfig, ...config },
  );
}

export function useRecentExtrinsics(count = 20, config?: SWRConfiguration) {
  return useSWR<ExtrinsicInfo[], Error>(['recent-extrinsics', count], () => getRecentExtrinsics(count), {
    ...defaultConfig,
    refreshInterval: 6000,
    ...config,
  });
}

export function useAccount(address: string | null, config?: SWRConfiguration) {
  return useSWR<AccountInfo | null, Error>(
    address ? ['account', address] : null,
    () => (address ? getAccountInfo(address) : null),
    { ...defaultConfig, refreshInterval: 12000, ...config },
  );
}

export function useIsAuthorized(address: string | null, config?: SWRConfiguration) {
  return useSWR<boolean, Error>(
    address ? ['is-authorized', address] : null,
    () => (address ? isAccountAuthorized(address) : false),
    { ...defaultConfig, ...config },
  );
}

export function useCanonicalBalance(account: string | null, assetId: number, config?: SWRConfiguration) {
  return useSWR<string, Error>(
    account ? ['canonical-balance', account, assetId] : null,
    () => (account ? getCanonicalBalance(account, assetId) : '0'),
    { ...defaultConfig, refreshInterval: 12000, ...config },
  );
}

export function useAuthorities(config?: SWRConfiguration) {
  return useSWR<ValidatorInfo[], Error>('authorities', () => getAuthorities(), {
    ...defaultConfig,
    refreshInterval: 60000,
    ...config,
  });
}

export function useAuthorizedAccounts(config?: SWRConfiguration) {
  return useSWR<string[], Error>('authorized-accounts', () => getAuthorizedAccounts(), {
    ...defaultConfig,
    refreshInterval: 30000,
    ...config,
  });
}

export function useFormattedBalance(balance: string | null, decimals = 18): string {
  if (!balance) return '0';
  const balanceNum = BigInt(balance);
  const divisor = BigInt(10 ** decimals);
  const integerPart = balanceNum / divisor;
  const fractionalPart = balanceNum % divisor;
  return `${integerPart.toLocaleString()}.${fractionalPart.toString().padStart(decimals, '0').slice(0, 4)}`;
}

export function useShortAddress(address: string | null, chars = 6): string {
  if (!address) return '';
  if (address.length <= chars * 2 + 3) return address;
  return `${address.slice(0, chars)}...${address.slice(-chars)}`;
}

export function useRpcStats(config?: SWRConfiguration) {
  return useSWR<RealRpcStats | null, Error>('rpc-stats', () => fetchRpcStats(), {
    ...defaultConfig,
    refreshInterval: 5000,
    ...config,
  });
}

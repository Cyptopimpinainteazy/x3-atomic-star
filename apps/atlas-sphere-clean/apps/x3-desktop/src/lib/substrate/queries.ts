/**
 * Substrate Query Functions (ported from explorer)
 */

import { getApi } from './client';
import type { Header, SignedBlock } from '@polkadot/types/interfaces';
import type { Codec } from '@polkadot/types/types';

export interface BlockInfo {
  number: number;
  hash: string;
  parentHash: string;
  stateRoot: string;
  extrinsicsRoot: string;
  timestamp: number;
  author: string | null;
  extrinsicsCount: number;
}

export interface ExtrinsicInfo {
  hash: string;
  index: number;
  blockNumber: number;
  blockHash: string;
  section: string;
  method: string;
  args: Record<string, unknown>;
  signer: string | null;
  success: boolean;
  timestamp: number;
  fee?: string;
}

export interface NetworkStats {
  chain: string;
  nodeName: string;
  nodeVersion: string;
  blockNumber: number;
  blockHash: string;
  timestamp: number;
  peerCount: number;
  isSyncing: boolean;
  totalIssuance?: string;
  authorityCount: number;
}

export interface AccountInfo {
  address: string;
  nonce: number;
  free: string;
  reserved: string;
  frozen: string;
  isAuthorized: boolean;
  consumers: number;
  providers: number;
  sufficients: number;
}

export interface ValidatorInfo {
  address: string;
  isActive: boolean;
  isCurrentAuthor?: boolean;
  blocksProduced?: number;
}

/* ── Network ──────────────────────────────────────────────── */

export async function getNetworkStats(): Promise<NetworkStats> {
  const api = await getApi();
  const [chain, nodeName, nodeVersion, header, health, authorities] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version(),
    api.rpc.chain.getHeader(),
    api.rpc.system.health(),
    api.query.atlasKernel?.authorities?.() || api.query.aura?.authorities?.(),
  ]);
  const block = await api.rpc.chain.getBlock(header.hash);
  return {
    chain: chain.toString(),
    nodeName: nodeName.toString(),
    nodeVersion: nodeVersion.toString(),
    blockNumber: header.number.toNumber(),
    blockHash: header.hash.toHex(),
    timestamp: extractTimestamp(block),
    peerCount: health.peers.toNumber(),
    isSyncing: health.isSyncing.isTrue,
    authorityCount: (authorities as unknown as { length?: number })?.length || 0,
  };
}

/* ── Blocks ───────────────────────────────────────────────── */

export async function getBlock(blockId: number | string): Promise<BlockInfo | null> {
  const api = await getApi();
  let hash: string;
  if (typeof blockId === 'number') {
    const bh = await api.rpc.chain.getBlockHash(blockId);
    hash = bh.toHex();
  } else {
    hash = blockId;
  }
  const [signedBlock, header] = await Promise.all([
    api.rpc.chain.getBlock(hash),
    api.rpc.chain.getHeader(hash),
  ]);
  if (!signedBlock || !header) return null;
  return {
    number: header.number.toNumber(),
    hash: header.hash.toHex(),
    parentHash: header.parentHash.toHex(),
    stateRoot: header.stateRoot.toHex(),
    extrinsicsRoot: header.extrinsicsRoot.toHex(),
    timestamp: extractTimestamp(signedBlock),
    author: extractAuthor(header),
    extrinsicsCount: signedBlock.block.extrinsics.length,
  };
}

export async function getRecentBlocks(count = 10): Promise<BlockInfo[]> {
  const api = await getApi();
  const header = await api.rpc.chain.getHeader();
  const cur = header.number.toNumber();
  const blocks: BlockInfo[] = [];
  for (let i = cur; i >= Math.max(0, cur - count + 1); i--) {
    const b = await getBlock(i);
    if (b) blocks.push(b);
  }
  return blocks;
}

export async function subscribeNewHeads(callback: (h: Header) => void): Promise<() => void> {
  const api = await getApi();
  return api.rpc.chain.subscribeNewHeads(callback);
}

/* ── Extrinsics ───────────────────────────────────────────── */

export async function getBlockExtrinsics(blockId: number | string): Promise<ExtrinsicInfo[]> {
  const api = await getApi();
  let hash: string, blockNumber: number;
  if (typeof blockId === 'number') {
    const bh = await api.rpc.chain.getBlockHash(blockId);
    hash = bh.toHex();
    blockNumber = blockId;
  } else {
    hash = blockId;
    const h = await api.rpc.chain.getHeader(hash);
    blockNumber = h.number.toNumber();
  }
  const signedBlock = await api.rpc.chain.getBlock(hash);
  const timestamp = extractTimestamp(signedBlock);
  const events = await api.query.system.events.at(hash);
  return signedBlock.block.extrinsics.map((ext, index) => {
    const { method, section } = ext.method;
    const extrinsicEvents = (events as unknown as Array<{ phase: { asApplyExtrinsic?: { toNumber: () => number } } }>)
      .filter((e) => e.phase.asApplyExtrinsic?.toNumber() === index);
    const success = !extrinsicEvents.some(
      (e) =>
        (e as unknown as { event: { section: string; method: string } }).event?.section === 'system' &&
        (e as unknown as { event: { section: string; method: string } }).event?.method === 'ExtrinsicFailed',
    );
    return {
      hash: ext.hash.toHex(),
      index,
      blockNumber,
      blockHash: hash,
      section,
      method,
      args: ext.method.args.reduce((acc, arg, i) => {
        acc[`arg${i}`] = arg.toHuman();
        return acc;
      }, {} as Record<string, unknown>),
      signer: ext.signer?.toString() || null,
      success,
      timestamp,
    };
  });
}

export async function getRecentExtrinsics(count = 20): Promise<ExtrinsicInfo[]> {
  const api = await getApi();
  const header = await api.rpc.chain.getHeader();
  const cur = header.number.toNumber();
  const extrinsics: ExtrinsicInfo[] = [];
  let checked = 0;
  while (extrinsics.length < count && checked < 50) {
    const bn = cur - checked;
    if (bn < 0) break;
    const be = await getBlockExtrinsics(bn);
    extrinsics.push(...be.filter((e) => e.signer || (e.section !== 'timestamp' && e.section !== 'paraInherent')));
    checked++;
  }
  return extrinsics.slice(0, count);
}

/* ── X3 Kernel ─────────────────────────────────────────── */

export async function getAuthorizedAccounts(): Promise<string[]> {
  const api = await getApi();
  try {
    const accounts = await (api.rpc as any).atlasKernel.getAuthorizedAccounts();
    return (accounts as any).map((a: Codec) => a.toString());
  } catch {
    const entries = await api.query.atlasKernel?.authorizedAccounts?.entries?.();
    return (entries as any)?.map(([key]: any) => key.args[0].toString()) || [];
  }
}

export async function getAuthorities(): Promise<ValidatorInfo[]> {
  const api = await getApi();
  let authorities = await api.query.atlasKernel?.authorities?.();
  if (!authorities || (authorities as any).length === 0) {
    authorities = await api.query.aura?.authorities?.();
  }
  if (!authorities) return [];
  return (authorities as unknown as Codec[]).map((auth) => ({ address: auth.toString(), isActive: true }));
}

export async function isAccountAuthorized(address: string): Promise<boolean> {
  const api = await getApi();
  try {
    const result = await (api.rpc as any).atlasKernel.isAuthorized(address);
    return result.isTrue ?? false;
  } catch {
    const entry = await api.query.atlasKernel?.authorizedAccounts?.(address);
    return !!(entry as any)?.isSome;
  }
}

export async function getCanonicalBalance(account: string, assetId: number): Promise<string> {
  const api = await getApi();
  try {
    const balance = await (api.rpc as any).atlasKernel.getCanonicalBalance(account, assetId);
    return balance.toString();
  } catch {
    const balance = await api.query.atlasKernel?.canonicalLedger?.(account, assetId);
    return balance?.toString() || '0';
  }
}

export async function getAccountInfo(address: string): Promise<AccountInfo | null> {
  const api = await getApi();
  try {
    const accountData = await api.query.system.account(address);
    const authorized = await isAccountAuthorized(address);
    const data = accountData as any;
    return {
      address,
      nonce: data.nonce.toNumber(),
      free: data.data.free.toString(),
      reserved: data.data.reserved.toString(),
      frozen: data.data.frozen.toString(),
      isAuthorized: authorized,
      consumers: data.consumers.toNumber(),
      providers: data.providers.toNumber(),
      sufficients: data.sufficients.toNumber(),
    };
  } catch (e) {
    console.error('Error fetching account info:', e);
    return null;
  }
}

/* ── Helpers ──────────────────────────────────────────────── */

export interface RealRpcStats {
  total_requests: number;
  total_rejected: number;
  active_connections: number;
}

export async function fetchRpcStats(): Promise<RealRpcStats | null> {
  try {
    const api = await getApi();
    const data = await (api.rpc as any).x3Node.getRateLimitMetrics();
    return {
      total_requests: Number(data.total_requests.toString()),
      total_rejected: Number(data.total_rejected.toString()),
      active_connections: Number(data.active_connections.toString()),
    };
  } catch (e) {
    console.warn('Error fetching RPC stats (may not be supported on this node):', e);
    return null;
  }
}

/* ── Helpers ──────────────────────────────────────────────── */

function extractTimestamp(signedBlock: SignedBlock): number {
  for (const ext of signedBlock.block.extrinsics) {
    if (ext.method.section === 'timestamp' && ext.method.method === 'set') {
      const arg = ext.method.args[0];
      return Number((arg as any).toBigInt?.() || arg.toString());
    }
  }
  return Date.now();
}

function extractAuthor(header: Header): string | null {
  for (const log of header.digest.logs) {
    const logHuman = log.toHuman() as { PreRuntime?: [string, string] } | null;
    if (logHuman?.PreRuntime) {
      const [engine, data] = logHuman.PreRuntime;
      if (engine === 'aura') return `Authority-${parseInt(data, 16)}`;
    }
  }
  return null;
}

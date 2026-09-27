/**
 * Telemetry IPC bridge for the X3 Atomic Star OS shell.
 *
 * Consumes the REAL backend event streams and commands from
 * src-tauri/src/lib.rs — there are no mock generators here. The Rust backend
 * sources this data from sysinfo (CPU / memory / disks) and from the node's
 * JSON-RPC endpoint (finalized head / peers / sync state / identity), then
 * emits:
 *
 *   - `os:node_status`    (NodeStatusEvent — the read, or the typed error)
 *   - `os:system_metrics` (SystemMetricsData)
 *
 * Panels should depend ONLY on the subscribe helpers in this module; the
 * raw @tauri-apps/api details stay hidden here so the real events can be
 * swapped/replayed without touching panel components.
 *
 * A command that cannot read the node REJECTS with an IpcError; it never
 * resolves with a stand-in value. Panels must handle the rejection.
 */

// @ts-ignore - see apps/x3-desktop/src/ipc/tauri.ts for the same pattern
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
// @ts-ignore
import { listen as tauriListen } from '@tauri-apps/api/event';

/* ─── Payload types (must mirror structs in src-tauri/src/main.rs) ─── */

export type CpuMetrics = {
  usagePercent: number;
  cores: number;
  frequency: number;
};

export type MemoryMetrics = {
  used: number;
  total: number;
  usagePercent: number;
};

export type DiskMetrics = {
  name: string;
  used: number;
  total: number;
  usagePercent: number;
};

export type SystemMetricsData = {
  cpu: CpuMetrics;
  memory: MemoryMetrics;
  disk: DiskMetrics[];
  updatedAt: string;
};

/** The chain's finalized head, as the node itself reported it. */
export type FinalizedHead = {
  hash: string;
  number: number;
};

/** A live read of the chain the operator is running. */
export type NodeStatus = {
  running: boolean;
  finalized: FinalizedHead;
  peers: number;
  isSyncing: boolean;
  name: string;
  version: string;
  chain: string;
  /** `system_nodeRoles` when the node serves it; null when it does not report one. */
  role: string | null;
  observedAt: string;
};

export type ValidatorStatus = "healthy" | "syncing" | "isolated" | "stalled";

export type ValidatorHealth = {
  status: ValidatorStatus;
  finalized: FinalizedHead;
  peers: number;
  isSyncing: boolean;
  shouldHavePeers: boolean;
  role: string | null;
  observedAt: string;
  notes: string[];
};

/** The shape a rejected command carries: a stable code, a sentence, and the raw evidence. */
export type IpcError = {
  code: string;
  message: string;
  details: string | null;
};

/**
 * A push has no caller to return an error to, so `os:node_status` carries
 * either the read or the typed failure — never both, and never neither.
 */
export type NodeStatusEvent = {
  status: NodeStatus | null;
  error: IpcError | null;
};

export const EVENT_NODE_STATUS = 'os:node_status';
export const EVENT_SYSTEM_METRICS = 'os:system_metrics';

/** True only when running inside the Tauri webview. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** Typed invoke wrapper for backend commands. */
async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await tauriInvoke<T>(cmd, args);
  } catch (error) {
    console.error(`[telemetry] invoke('${cmd}') failed:`, error);
    throw error;
  }
}

/** Immediate (non-streaming) read of the latest system metrics snapshot. */
export function fetchSystemMetrics(): Promise<SystemMetricsData> {
  return invoke<SystemMetricsData>('get_system_metrics');
}

/** Immediate (non-streaming) read of the chain. Rejects when the node cannot be read. */
export function fetchNodeStatus(): Promise<NodeStatus> {
  return invoke<NodeStatus>('get_node_status');
}

/** Validator health: the same reads as {@link fetchNodeStatus}, plus a verdict. */
export function fetchValidatorHealth(): Promise<ValidatorHealth> {
  return invoke<ValidatorHealth>('get_validator_health');
}

/** Subscribe to pushes of the system metrics snapshot. Returns an unlisten fn. */
export async function subscribeSystemMetrics(
  cb: (metrics: SystemMetricsData) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const unlisten = await tauriListen<SystemMetricsData>(EVENT_SYSTEM_METRICS, (event) => {
    cb(event.payload);
  });
  return unlisten;
}

/** Subscribe to pushes of the node status (or its failure). Returns an unlisten fn. */
export async function subscribeNodeStatus(
  cb: (event: NodeStatusEvent) => void,
): Promise<() => void> {
  if (!isTauri()) return () => {};
  const unlisten = await tauriListen<NodeStatusEvent>(EVENT_NODE_STATUS, (event) => {
    cb(event.payload);
  });
  return unlisten;
}

/* ─── Formatting helpers (shared by panels) ─── */

/** Format a byte count into a human-readable IEC string. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return 'n/a';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value.toFixed(value < 10 && i > 0 ? 1 : 0)} ${units[i]}`;
}

export function formatPercent(value: number): string {
  if (!Number.isFinite(value)) return 'n/a';
  return `${value.toFixed(1)}%`;
}

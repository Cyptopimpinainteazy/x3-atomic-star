import React, { useCallback, useEffect, useState } from "react";
import {
  DiskMetrics,
  EVENT_NODE_STATUS,
  EVENT_SYSTEM_METRICS,
  IpcError,
  NodeStatus,
  SystemMetricsData,
  fetchNodeStatus,
  fetchSystemMetrics,
  formatBytes,
  formatPercent,
  isTauri,
  subscribeNodeStatus,
  subscribeSystemMetrics,
} from "../lib/telemetry";

type ConnectionState =
  | { type: "connecting" }
  | { type: "live" }
  | { type: "offline"; reason: string };

/**
 * Live operator telemetry for the X3 Atomic Star OS shell.
 *
 * This panel is NOT fed by mock generators — it renders the real snapshots
 * the Rust backend publishes over `os:node_status` / `os:system_metrics`
 * (sysinfo CPU/memory/disk + node JSON-RPC peer state). If no Tauri host is
 * present (e.g. plain browser dev) it shows an explicit offline state instead
 * of inventing values.
 */
export function LiveTelemetryPanel() {
  const [metrics, setMetrics] = useState<SystemMetricsData | null>(null);
  const [node, setNode] = useState<NodeStatus | null>(null);
  const [nodeError, setNodeError] = useState<IpcError | null>(null);
  const [conn, setConn] = useState<ConnectionState>({ type: "connecting" });

  useEffect(() => {
    if (!isTauri()) {
      setConn({ type: "offline", reason: "not running inside Tauri (browser preview)" });
      return;
    }

    let disposed = false;
    const unlisteners: (() => void)[] = [];

    (async () => {
      // Seed with the current snapshots, then keep listening for pushes.
      try {
        const metricsSnapshot = await fetchSystemMetrics();
        if (disposed) return;
        setMetrics(metricsSnapshot);
        setConn({ type: "live" });
      } catch {
        if (disposed) return;
        setConn({ type: "offline", reason: "backend metrics not reachable" });
        return;
      }

      // The system metrics are the app's own; the node read can fail on its
      // own, and a failed read is shown as a failure rather than as empty data.
      try {
        const nodeStatus = await fetchNodeStatus();
        if (disposed) return;
        setNode(nodeStatus);
        setNodeError(null);
      } catch (error) {
        if (disposed) return;
        setNode(null);
        setNodeError(error as IpcError);
      }

      try {
        const u1 = await subscribeSystemMetrics((m) => {
          setMetrics(m);
          setConn({ type: "live" });
        });
        const u2 = await subscribeNodeStatus((event) => {
          setNode(event.status);
          setNodeError(event.error);
          setConn({ type: "live" });
        });
        if (!disposed) {
          unlisteners.push(u1, u2);
        } else {
          u1();
          u2();
        }
      } catch (err) {
        if (!disposed) {
          setConn({
            type: "offline",
            reason: err instanceof Error ? err.message : String(err),
          });
        }
      }
    })();

    return () => {
      disposed = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);

  return (
    <div style={{ padding: 24, fontFamily: "Inter, sans-serif", lineHeight: 1.5 }}>
      <h1>Live Operator Telemetry</h1>
      <p>
        Real CPU / memory / storage from <code>sysinfo</code> and node state from the
        local RPC endpoint — streamed via Tauri events every ~5s. No mock data.
      </p>

      {conn.type === "connecting" && <p>Connecting to backend telemetry…</p>}
      {conn.type === "offline" && (
        <p style={{ color: "#b45309" }}>
          Backend offline: {conn.reason}. Data will appear once the OS shell backend is
          running (node + swarm reachable).
        </p>
      )}

      {metrics && (
        <div style={{ display: "grid", gap: 16, gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))" }}>
          <GaugeCard title={`CPU ${formatPercent(metrics.cpu.usagePercent)}`} percent={metrics.cpu.usagePercent}>
            <div>
              {metrics.cpu.cores} cores · {metrics.cpu.frequency} MHz
            </div>
            <div style={{ fontSize: 12, color: "#555" }}>Last update: {metrics.updatedAt}</div>
          </GaugeCard>

          <GaugeCard title={`Memory ${formatPercent(metrics.memory.usagePercent)}`} percent={metrics.memory.usagePercent}>
            <div>
              {formatBytes(metrics.memory.used)} used / {formatBytes(metrics.memory.total)} total
            </div>
            <div style={{ fontSize: 12, color: "#555" }}>Last update: {metrics.updatedAt}</div>
          </GaugeCard>

          <StorageCard disks={metrics.disk} />
          <NodeCard node={node} error={nodeError} />
          <Button refresh={() => refreshOnce(setMetrics, setNode, setNodeError, setConn)} />
        </div>
      )}

      {!metrics && (
        <p style={{ marginTop: 16, color: "#6b7280" }}>
          Waiting for the first system-metrics snapshot from the OS shell backend…
        </p>
      )}
    </div>
  );
}

async function refreshOnce(
  setMetrics: React.Dispatch<React.SetStateAction<SystemMetricsData | null>>,
  setNode: React.Dispatch<React.SetStateAction<NodeStatus | null>>,
  setNodeError: React.Dispatch<React.SetStateAction<IpcError | null>>,
  setConn: React.Dispatch<React.SetStateAction<ConnectionState>>,
) {
  if (!isTauri()) return;
  try {
    setMetrics(await fetchSystemMetrics());
    setConn({ type: "live" });
  } catch {
    setConn({ type: "offline", reason: "backend metrics not reachable" });
    return;
  }
  try {
    setNode(await fetchNodeStatus());
    setNodeError(null);
  } catch (error) {
    setNode(null);
    setNodeError(error as IpcError);
  }
}

function Button({ refresh }: { refresh: () => void }) {
  return (
    <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
      <h3>Controls</h3>
      <button onClick={refresh} style={{ padding: "8px 14px" }}>
        Refresh now
      </button>
      <p style={{ fontSize: 12, color: "#555", marginTop: 8 }}>
        Panels update automatically every ~5s from the backend event stream.
      </p>
    </div>
  );
}

function GaugeCard({
  title,
  percent,
  children,
}: {
  title: string;
  percent: number;
  children: React.ReactNode;
}) {
  const clamped = Math.max(0, Math.min(100, percent));
  return (
    <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
      <h3 style={{ marginTop: 0 }}>{title}</h3>
      <svg width="100%" height={18} viewBox="0 0 240 18" preserveAspectRatio="none">
        <rect x={0} y={0} width={240} height={18} rx={9} fill="#ececec" />
        <rect
          x={0}
          y={0}
          width={(clamped / 100) * 240}
          height={18}
          rx={9}
          fill={clamped > 90 ? "#dc2626" : clamped > 70 ? "#f59e0b" : "#16a34a"}
        />
      </svg>
      {children}
    </div>
  );
}

function StorageCard({ disks }: { disks: DiskMetrics[] }) {
  return (
    <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
      <h3 style={{ marginTop: 0 }}>Storage</h3>
      {disks.length === 0 && <p>No disk reported by sysinfo.</p>}
      {disks.map((d) => (
        <div key={d.name} style={{ marginBottom: 12 }}>
          <GaugeRow label={d.name} percent={d.usagePercent} detail={`${formatBytes(d.used)} / ${formatBytes(d.total)}`} />
        </div>
      ))}
    </div>
  );
}

function GaugeRow({ label, percent, detail }: { label: string; percent: number; detail: string }) {
  const clamped = Math.max(0, Math.min(100, percent || 0));
  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 13 }}>
        <strong>{label}</strong>
        <span>{formatPercent(percent || 0)}</span>
      </div>
      <svg width="100%" height={10} viewBox="0 0 240 10" preserveAspectRatio="none" style={{ margin: "2px 0" }}>
        <rect x={0} y={0} width={240} height={10} rx={5} fill="#ececec" />
        <rect
          x={0}
          y={0}
          width={(clamped / 100) * 240}
          height={10}
          rx={5}
          fill={clamped > 90 ? "#dc2626" : clamped > 70 ? "#f59e0b" : "#3b82f6"}
        />
      </svg>
      <div style={{ fontSize: 12, color: "#555" }}>{detail}</div>
    </div>
  );
}

function NodeCard({ node, error }: { node: NodeStatus | null; error: IpcError | null }) {
  return (
    <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
      <h3 style={{ marginTop: 0 }}>Node Status</h3>
      {node ? (
        <div>
          <p>
            Status:{" "}
            <span style={{ color: node.isSyncing ? "#b45309" : "#16a34a" }}>
              {node.isSyncing ? "syncing" : "running"}
            </span>
          </p>
          <p>
            {node.name} {node.version}
          </p>
          <p>Chain: {node.chain}</p>
          <p>Role: {node.role ?? "not reported"}</p>
          <p>Peers: {node.peers}</p>
          <p>
            Finalized: #{node.finalized.number}{" "}
            <code style={{ fontSize: 11 }}>
              {node.finalized.hash.slice(0, 10)}…{node.finalized.hash.slice(-6)}
            </code>
          </p>
          <p style={{ fontSize: 12, color: "#555" }}>Last check: {node.observedAt}</p>
        </div>
      ) : error ? (
        <div>
          <p style={{ color: "#b45309" }}>Node unreachable: {error.code}</p>
          <p style={{ fontSize: 12, color: "#555" }}>{error.details ?? error.message}</p>
        </div>
      ) : (
        <p style={{ color: "#6b7280" }}>No node status received yet.</p>
      )}
      <p style={{ fontSize: 12, color: "#555", marginTop: 8 }}>
        Read from the node&apos;s JSON-RPC endpoint by the Rust backend; a node that does not answer
        is reported here, never guessed at.
      </p>
    </div>
  );
}

/* ——————— end file ——————— */

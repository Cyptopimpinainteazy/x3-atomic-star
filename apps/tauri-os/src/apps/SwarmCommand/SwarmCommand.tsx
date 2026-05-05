import React, { useEffect, useState } from "react";

type TaskRecord = {
  id: string;
  title: string;
  feature: string;
  agent: string;
  permission_tier: string;
  status: string;
  approval_required: string;
  risk: string;
};

type HealthStatus = {
  service: string;
  status: string;
  mode: string;
  agents_enabled: boolean;
  kill_switch: boolean;
};

export function SwarmCommand() {
  const [health, setHealth] = useState<HealthStatus | null>(null);
  const [tasks, setTasks] = useState<TaskRecord[]>([]);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    try {
      const statusResp = await fetch("http://127.0.0.1:8787/health");
      if (!statusResp.ok) throw new Error("Health endpoint unavailable");
      setHealth(await statusResp.json());

      const tasksResp = await fetch("http://127.0.0.1:8787/tasks");
      if (!tasksResp.ok) throw new Error("Tasks endpoint unavailable");
      setTasks(await tasksResp.json());
      setError(null);
    } catch (err) {
      setError((err as Error).message);
      setHealth(null);
      setTasks([]);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  const backendConnected = !!health && !error;
  const latestTask = tasks[0] ?? null;

  return (
    <div style={{ padding: 24, fontFamily: "Inter, sans-serif", lineHeight: 1.6 }}>
      <h1>X3 Swarm Command</h1>
      <p>The SwarmCommand panel shows swarm status, tasks, approvals, and health for the local build.</p>

      <section style={{ marginBottom: 24 }}>
        <h2>Backend Status</h2>
        {backendConnected ? (
          <div>
            <p>Service: {health?.service}</p>
            <p>Status: {health?.status}</p>
            <p>Mode: {health?.mode}</p>
            <p>Agents enabled: {health?.agents_enabled ? "yes" : "no"}</p>
            <p>Kill switch: {health?.kill_switch ? "ENGAGED" : "clear"}</p>
            <button onClick={refresh} style={{ marginTop: 12, padding: "8px 14px" }}>
              Refresh
            </button>
          </div>
        ) : (
          <div>
            <p>Mode: VIEW_ONLY</p>
            <p>Backend: not connected</p>
            <p>Blocker: x3-swarm-api unavailable</p>
            <p>Next action: run <code>scripts/swarm/swarm_up.sh</code></p>
            {error ? <p style={{ color: "#c00" }}>Error: {error}</p> : null}
          </div>
        )}
      </section>

      <div style={{ display: "grid", gap: 16, gridTemplateColumns: "repeat(auto-fit, minmax(280px, 1fr))" }}>
        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Agent Roster</h3>
          <ul>
            <li>RepoScanner</li>
            <li>FeatureMapper</li>
            <li>TestBuilder</li>
            <li>Integrator</li>
            <li>BuildFixer</li>
            <li>WiringInspector</li>
            <li>Auditor</li>
            <li>Breaker</li>
            <li>Fixer</li>
            <li>ReadinessReporter</li>
            <li>Benchmark</li>
            <li>Marketing</li>
            <li>Grant</li>
            <li>ApprovalGate</li>
          </ul>
        </div>

        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Task Queue</h3>
          {tasks.length === 0 ? (
            <p>No tasks loaded. Generate a queue by running <code>cargo run -p x3-readiness -- swarm-tasks --out reports</code>.</p>
          ) : (
            <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
              {tasks.slice(0, 5).map((task) => (
                <li key={task.id} style={{ marginBottom: 12, paddingBottom: 8, borderBottom: "1px solid #eee" }}>
                  <strong>{task.title}</strong>
                  <div>{task.feature} · {task.agent} · {task.risk}</div>
                  <div>Approval: {task.approval_required}</div>
                </li>
              ))}
            </ul>
          )}
        </div>

        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Approval Gate</h3>
          <p>Latest approval required: {latestTask?.approval_required ?? "None"}</p>
          <p>Pending approvals: {tasks.filter((task) => task.status === "NeedsApproval").length}</p>
        </div>

        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Scoreboard</h3>
          <p>Backend mode: {health?.mode ?? "unknown"}</p>
          <p>Task count: {tasks.length}</p>
        </div>

        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Memory Viewer</h3>
          <p>Memory entries are stored in <code>data/agent-memory</code>.</p>
          <p>Implement memory load and review to surface agent learnings.</p>
        </div>

        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Kill Switch</h3>
          <p>{health?.kill_switch ? "Kill switch engaged" : "Kill switch clear"}</p>
          <p>Use the API kill switch endpoint to halt swarm execution if needed.</p>
        </div>

        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Swarm Health</h3>
          <p>{backendConnected ? "Backend reachable" : "Backend unreachable"}</p>
          <p>Report file: <code>reports/swarm_health_report.md</code></p>
        </div>

        <div style={{ padding: 16, border: "1px solid #ddd", borderRadius: 12 }}>
          <h3>Feature Gaps</h3>
          <p>Feature gap report: <code>reports/feature_gap_report.md</code></p>
          <p>Missing tests: <code>reports/missing_tests_report.md</code></p>
        </div>
      </div>
    </div>
  );
}

}

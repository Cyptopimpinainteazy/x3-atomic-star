//! The background tick that keeps the panels live.
//!
//! It reads the machine with `sysinfo` and the chain with the same
//! [`ChainClient`](crate::chain::ChainClient) the commands use — there is no
//! second, easier path to the node for the event stream. A push has no caller to
//! return an error to, so a failed chain read is emitted as a failure envelope
//! (see [`NodeStatusEvent`](crate::models::NodeStatusEvent)); the panel is never
//! handed a value it cannot tell apart from a live one.

use crate::commands::{node_status, observed_at};
use crate::error::IpcError;
use crate::models::{DiskMetrics, NodeStatusEvent, SystemMetricsData};
use crate::state::{read_snapshot, write_snapshot, ConsoleState};
use std::time::Duration;
use sysinfo::{Disks, System};
use tauri::Emitter;

/// Event names the webview subscribes to (`apps/tauri-os/src/lib/telemetry.ts`).
pub const EVENT_NODE_STATUS: &str = "os:node_status";
pub const EVENT_SYSTEM_METRICS: &str = "os:system_metrics";

/// How often the panels are refreshed. One tick drives both streams so the node
/// panel and the hardware panel always describe the same instant.
pub const TICK: Duration = Duration::from_secs(5);

/// Read CPU, memory and disk. Every field comes from `sysinfo`; none is derived
/// from another (the disk fields used to be filled in from RAM numbers, which
/// reported memory as storage).
pub fn read_system_metrics(sys: &System, disks: &mut Disks) -> SystemMetricsData {
    // sysinfo 0.31: global_cpu_usage() returns f32 directly; per-core
    // usage/frequency come from the Cpu entries returned by System::cpus().
    let cpu_usage = sys.global_cpu_usage();
    let total_memory = sys.total_memory();
    let used_memory = sys.used_memory();

    let cores = sys.cpus().len() as u32;
    let frequency = sys.cpus().first().map(|cpu| cpu.frequency()).unwrap_or(0);

    disks.refresh();
    let mut disk_metrics: Vec<DiskMetrics> = Vec::new();
    for disk in disks.list() {
        let total = disk.total_space();
        let used = total.saturating_sub(disk.available_space());
        let usage_percent = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        disk_metrics.push(DiskMetrics {
            name: disk.name().to_string_lossy().to_string(),
            used,
            total,
            usage_percent: usage_percent as f32,
        });
    }
    if disk_metrics.is_empty() {
        // No physical disk reported (an unusual container); keep the stream
        // valid, and say so with a zero-sized entry rather than invented bytes.
        disk_metrics.push(DiskMetrics {
            name: "System".to_owned(),
            used: 0,
            total: 0,
            usage_percent: 0.0,
        });
    }

    SystemMetricsData {
        cpu: crate::models::CpuMetrics {
            usage_percent: cpu_usage,
            cores,
            frequency,
        },
        memory: crate::models::MemoryMetrics {
            used: used_memory * 1024,
            total: total_memory * 1024,
            usage_percent: if total_memory > 0 {
                (used_memory as f32 / total_memory as f32) * 100.0
            } else {
                0.0
            },
        },
        disk: disk_metrics,
        updated_at: observed_at(),
    }
}

/// Start the tick. Returns immediately; the loop runs on Tauri's async runtime
/// for the life of the app.
pub fn spawn(app: tauri::AppHandle, state: ConsoleState) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(TICK).await;

            let metrics = state.refresh_metrics();
            write_snapshot(&state.system_metrics, metrics);

            let status_event = match node_status(state.chain.as_ref()).await {
                Ok(status) => NodeStatusEvent::ok(status),
                Err(error) => NodeStatusEvent::failed(IpcError::from(error)),
            };
            let _ = app.emit(EVENT_NODE_STATUS, status_event);

            let metrics_snapshot = read_snapshot(&state.system_metrics);
            let _ = app.emit(EVENT_SYSTEM_METRICS, metrics_snapshot);
        }
    });
}

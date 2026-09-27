//! Shared state for the operator console.
//!
//! Every service the console talks to is a trait object created once at
//! startup, so a command cannot reach for a global, a literal URL, or a cache
//! and call the result live. The metrics snapshots are guarded by locks that
//! recover from poisoning instead of panicking: each value is rewritten
//! wholesale on the next tick, so a panicking writer cannot leave a
//! half-updated snapshot for a reader to interpret.

use crate::chain::{ChainClient, RpcChainClient};
use crate::models::SystemMetricsData;
use crate::monitor::read_system_metrics;
use crate::probe::{HttpProbe, ServiceProbe};
use crate::swarm::{HttpSwarmClient, SwarmClient};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use sysinfo::{Disks, System};

/// What every command and the background monitor share.
#[derive(Clone)]
pub struct ConsoleState {
    /// The node the operator is running.
    pub chain: Arc<dyn ChainClient>,
    /// The swarm API that holds the task queue.
    pub swarm: Arc<dyn SwarmClient>,
    /// The probe used for the local service reachability panel.
    pub probe: Arc<dyn ServiceProbe>,
    pub system_metrics: Arc<RwLock<SystemMetricsData>>,
    system: Arc<Mutex<System>>,
    disks: Arc<Mutex<Disks>>,
}

impl ConsoleState {
    pub fn new(
        chain: Arc<dyn ChainClient>,
        swarm: Arc<dyn SwarmClient>,
        probe: Arc<dyn ServiceProbe>,
    ) -> Self {
        let mut system = System::new_all();
        system.refresh_all();
        let mut disks = Disks::new_with_refreshed_list();
        let initial = read_system_metrics(&system, &mut disks);
        Self {
            chain,
            swarm,
            probe,
            system_metrics: Arc::new(RwLock::new(initial)),
            system: Arc::new(Mutex::new(system)),
            disks: Arc::new(Mutex::new(disks)),
        }
    }

    /// The endpoints the operator named, or the local defaults.
    pub fn from_env() -> Self {
        Self::new(
            Arc::new(RpcChainClient::from_env()),
            Arc::new(HttpSwarmClient::from_env()),
            Arc::new(HttpProbe::new()),
        )
    }

    /// Re-read the machine. Called on the monitor's tick, never on the UI thread.
    pub fn refresh_metrics(&self) -> SystemMetricsData {
        with_lock(&self.system, |system| {
            system.refresh_cpu_all();
            system.refresh_memory();
            with_lock(&self.disks, |disks| read_system_metrics(system, disks))
        })
    }
}

/// Read a snapshot, recovering a poisoned lock instead of panicking.
pub fn read_snapshot<T: Clone>(lock: &RwLock<T>) -> T {
    match lock.read() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    }
}

/// Replace a snapshot, recovering a poisoned lock instead of panicking.
pub fn write_snapshot<T>(lock: &RwLock<T>, value: T) {
    match lock.write() {
        Ok(mut guard) => *guard = value,
        Err(poisoned) => *poisoned.into_inner() = value,
    }
}

/// Run `body` with the guarded value, recovering a poisoned lock instead of
/// panicking. `sysinfo`'s handles are plain measurements: the worst a poisoned
/// lock can mean is that the previous holder panicked mid-refresh, and the next
/// tick overwrites every field anyway.
fn with_lock<T, R>(lock: &Mutex<T>, body: impl FnOnce(&mut T) -> R) -> R {
    let mut guard: MutexGuard<'_, T> = match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    body(&mut guard)
}

# X3 Swarm Orchestra

This is the unified, single-folder orchestrator for the **X3 Swarm & Orchestra Platform**. We are shipping in two days, so everything is tied together right here, tight as hell.

## What's Included?

By running this master folder, you tie together the following micro-architectures into a single multi-agent system:

1. **GPU Swarm & Validators (TPS Priority)**
   - Located in `./validators/`
   - Drives ultra-fast scaling with multi-VM nodes utilizing Nvidia configurations. The primary task is maximizing TPS.

2. **Blockchain Optimization & Security**
   - The nodes are sandboxed, and we run autonomic safety health checks via `./ai-agents/python-swarm`. Autonomic ops will restart failed components and monitor the GPU heartbeat to ensure optimal chain optimization and security.

3. **Arbitrage (Quantum-Swarm & Atomic Swap)**
   - Located in `./arbitrage/`
   - Strategy Agent for extracting flash-loan opportunities and scanning prices cross-chain. 

4. **AI & User Paid Features (Marketing, Coding, Video)**
   - Includes **Ralph** (coding), **PostAutomation** (marketing), and **Swarm-Media** (video creation). These represent the premium value-added features on the network.

## Quick Start (Shipping Mode)

To fire up the entire platform on your local container engine:

```bash
make start
```

### Benchmarking the TPS

To test the GPU cluster and see how tight and optimized the blockchain is:
```bash
make tps-bench
```

### Viewing Arbitrage Logs
```bash
make arbitrage
```

## Structure

```text
/x3-swarm-orchestra
├── Makefile                # Master commands
├── docker-compose.yml      # Master container definitions
├── validators/             # Symlink -> crates/x3-gpu-validator-swarm
├── arbitrage/              # Symlink -> crates/quantum-swarm & crates/atomic-swap-orchestrator
├── ai-agents/              # Symlink -> swarm/ (autonomics) & ralph
├── marketing/              # Symlink -> super-ide/apps/PostAutomation-AIAgent 
├── video-creation/         # Symlink -> crates/swarm-media
└── config/                 # Master configs
```

## Evolution Lifecycle
This follows the **OpenSpec** BMAD workflow proposal. The central oversight “conductor” runs from `docker-compose` combining decentralized AI agents across all the links provided here. 

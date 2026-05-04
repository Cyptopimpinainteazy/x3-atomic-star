# x3-Chain Block Visualization Integration

Display neon-styled block visualizations in your terminal when blocks are finalized on the x3-chain.

## Quick Start

### Compact Display (Recommended for logs)
```bash
python3 scripts/block_display.py -c 8 "FINALIZED BLOCKS"
```

Output:
```
FINALIZED BLOCKS

┌──┐┌──┐┌──┐┌──┐┌──┐┌──┐┌──┐┌──┐
│1││2││3││4││5││6││7││8│
└──┘└──┘└──┘└──┘└──┘└──┘└──┘└──┘
```

### Full Display (For dashboards/UIs)
```bash
python3 scripts/block_display.py 8 "FINALIZED BLOCKS"
```

## Integration Examples

### 1. Bash Integration (in run-dev-node.sh or similar)

```bash
#!/bin/bash

# Your node startup code here
cargo run -p x3-chain-node --release &
NODE_PID=$!

# Display blocks as they're finalized
echo "[$(date)] Node started with PID $NODE_PID"

# Periodically show block status
while kill -0 $NODE_PID 2>/dev/null; do
    sleep 5
    # Get current block count from logs or RPC
    BLOCK_COUNT=$(curl -s http://localhost:9944 -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"chain_getHeader","params":[],"id":1}' | jq '.result.number' | xargs printf '%d')
    
    if [ ! -z "$BLOCK_COUNT" ]; then
        python3 scripts/block_display.py -c 8 "BLOCKS FINALIZED: $BLOCK_COUNT"
    fi
done
```

### 2. Rust Integration (in node/src/logging.rs or main.rs)

```rust
use std::process::Command;

fn display_block_status(block_count: u32) {
    let num_blocks = std::cmp::min(block_count, 8);
    let output = Command::new("python3")
        .args(&[
            "scripts/block_display.py",
            "-c",
            &num_blocks.to_string(),
            "BLOCK FINALIZED",
        ])
        .output()
        .expect("Failed to display blocks");
    
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

// Call this in your block finality handler
pub fn on_block_finalized(block_number: u32) {
    display_block_status(block_number);
    log::info!("Block {} finalized", block_number);
}
```

### 3. Log Pipe Integration

Pipe your node logs and trigger block display:

```bash
./run-dev-node.sh 2>&1 | tee >(
    grep -oP '(?<=Finalized block: )\d+' | while read block; do
        python3 scripts/block_display.py -c $((block % 8 + 1)) "BLOCK $block"
    done
) | grep -v "block visualization"
```

### 4. Docker Integration

Add to your Dockerfile or docker-compose.yml:

```dockerfile
# Install Python for block visualization
RUN apt-get update && apt-get install -y python3

# Copy scripts
COPY scripts/block_display.py /app/scripts/

# Entrypoint script
COPY scripts/entrypoint-with-viz.sh /app/scripts/
RUN chmod +x /app/scripts/entrypoint-with-viz.sh

ENTRYPOINT ["/app/scripts/entrypoint-with-viz.sh"]
```

### 5. Monitoring Dashboard Script

```bash
#!/bin/bash

# monitoring/block_display_monitor.sh
# Monitors block finalization and displays neon blocks

LOG_FILE="logs/node.log"
LAST_BLOCK=0

tail -f "$LOG_FILE" | while read line; do
    if [[ $line =~ "Finalized block: ([0-9]+)" ]]; then
        CURRENT_BLOCK="${BASH_REMATCH[1]}"
        
        if [ "$CURRENT_BLOCK" -gt "$LAST_BLOCK" ]; then
            clear
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo "x3-Chain Block Finalization Monitor"
            echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
            echo "Timestamp: $(date)"
            echo "Block: $CURRENT_BLOCK"
            echo ""
            
            # Show last 8 blocks or fewer if less than 8 blocks mined
            BLOCKS_TO_SHOW=$((CURRENT_BLOCK > 8 ? 8 : CURRENT_BLOCK))
            python3 scripts/block_display.py -c "$BLOCKS_TO_SHOW" "FINALIZED BLOCKS"
            
            LAST_BLOCK="$CURRENT_BLOCK"
        fi
    fi
done
```

## Command-Line Options

### block_display.py

```
Usage: python3 scripts/block_display.py [OPTIONS] [NUM_BLOCKS] [HEADER]

Options:
  -c, --compact     Use compact display (recommended for logs)
  NUM_BLOCKS        Number of blocks to display (1-8, default: 8)
  HEADER            Optional header text

Examples:
  python3 scripts/block_display.py                    # Full display, 8 blocks
  python3 scripts/block_display.py 4                  # Full display, 4 blocks
  python3 scripts/block_display.py -c 8               # Compact, 8 blocks
  python3 scripts/block_display.py -c 8 "FINALIZED"   # Compact with header
  python3 scripts/block_display.py 8 "MY HEADER"      # Full with header
```

### block_visualizer.py (Alternative)

```
Usage: python3 scripts/block_visualizer.py [NUM_BLOCKS]

Examples:
  python3 scripts/block_visualizer.py        # 8 blocks
  python3 scripts/block_visualizer.py 4      # 4 blocks
```

## Color Scheme

The blocks use a rainbow gradient of neon colors:

1. **Block 1**: Bright Yellow (#FFFF00)
2. **Block 2**: Orange (#FF8800)
3. **Block 3**: Bright Green (#00FF00)
4. **Block 4**: Bright Cyan (#00FFFF)
5. **Block 5**: Bright Blue (#0088FF)
6. **Block 6**: Magenta (#FF00FF)
7. **Block 7**: Bright Red (#FF0000)
8. **Block 8**: Bright Green (#00FF00)

## Terminal Requirements

- **Color Support**: 256-color terminal or better
- **Unicode Support**: For box-drawing characters (┌, ─, ┐, etc.)
- **Tested Terminals**:
  - Linux: bash, zsh, fish
  - macOS: Terminal.app, iTerm2
  - Windows: Windows Terminal, WSL2

## Performance Notes

- Minimal CPU overhead (single Python process per display)
- Safe to call frequently from logs
- No external dependencies beyond Python 3

## Troubleshooting

### Colors not showing
- Enable 256-color support: `export TERM=xterm-256color`
- Check terminal settings for color support

### Box characters not displaying
- Ensure UTF-8 encoding: `export LC_ALL=en_US.UTF-8`
- Use a Unicode-capable font (Monaco, Consolas, Source Code Pro)

### Script not found
- Add scripts to PATH: `export PATH="$PATH:$(pwd)/scripts"`
- Or use absolute paths

## Implementation Checklist

- [ ] Copy `scripts/block_display.py` to your project
- [ ] Make executable: `chmod +x scripts/block_display.py`
- [ ] Test: `python3 scripts/block_display.py -c 8`
- [ ] Integrate into your node startup script
- [ ] Test with actual block finalization
- [ ] Add to CI/CD pipeline if needed

## License

Part of the x3-Chain project. Same license as main project.

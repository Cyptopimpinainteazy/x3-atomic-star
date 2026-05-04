# X3 Chain Ollama Streaming Tool-Calling Wrapper

**Production-ready streaming tool-calling for X3 Chain security analysis, code auditing, and AI-driven development workflows.**

## What Is This?

A Python wrapper around Ollama's tool-calling API that enables X3 Chain developers to:

- **Analyze Rust smart contracts** for security vulnerabilities
- **Audit fraud proofs** for correctness
- **Validate cross-VM** configurations
- **Run multi-step reasoning** with agent loops
- **Stream real-time feedback** as the model thinks and acts

Supports single tool calls, parallel tool invocation, and multi-turn agent loops with streaming or non-streaming modes.

## Why Streaming?

```
❌ Non-streaming: [waiting.....................] User sees nothing for 10 seconds
✅ Streaming:     [generating... thinking... calling tools... done!] Instant feedback
```

Real-time output makes audits feel fast, allows user interruption, and shows model reasoning.

## Quick Start (3 Lines)

```python
from x3_chain_ollama_tools import StreamingToolCaller

caller = StreamingToolCaller(model="qwen2.5-coder:7b")
result = caller.call_single_tool("Analyze witness_v1.rs", tools=[your_analysis_function])
print(result.final_response)
```

## Installation

```bash
# Install ollama client
pip install ollama -U

# Download the model (if not already present)
ollama pull qwen2.5-coder:7b

# Verify ollama is running
ollama serve  # In separate terminal
```

## Files Included

| File | Purpose |
|------|---------|
| **x3_chain_ollama_tools.py** | Core implementation (StreamingToolCaller, X3ChainAnalyzer) |
| **STREAMING_PATTERNS_GUIDE.py** | 6 complete code patterns: streaming vs non-streaming |
| **X3_OLLAMA_USAGE_GUIDE.py** | Real-world X3 Chain examples |
| **test_x3_ollama_tools.py** | Test suite (7 tests, copy-paste examples) |
| **X3_CHAIN_OLLAMA_DEPLOYMENT_GUIDE.py** | Production deployment guide |

## Core Concepts

### StreamingToolCaller

Main wrapper for tool-calling workflows.

```python
caller = StreamingToolCaller(
    model="qwen2.5-coder:7b",      # Model to use
    render_thinking=True,           # Show thinking process
    render_content=True,            # Show responses
    render_tool_calls=True,         # Show tools as used
)
```

### Three Tool-Calling Patterns

**1. Single Tool Call** - Simple, focused analysis

```python
result = caller.call_single_tool(
    "Analyze this file for security",
    tools=[analyze_security],
    stream=True  # Real-time feedback
)
```

**2. Parallel Tools** - Multiple tools simultaneously

```python
result = caller.call_parallel_tools(
    "Audit fraud_proofs module",
    tools=[check_security, check_performance, verify_proofs],
    stream=True
)
```

**3. Agent Loop** - Multi-turn reasoning with automatic tool decisions

```python
result = caller.agent_loop(
    "What is (5+3)*2?",
    tools=[add, multiply],
    max_iterations=5,
    stream=True
)
```

### ToolResult

Every tool call returns a `ToolResult`:

```python
@dataclass
class ToolResult:
    thinking: str              # Model's reasoning (if available)
    content: str               # Initial response
    tool_calls_made: list      # [(tool_name, args), ...]
    final_response: str        # Final synthesized answer
```

### X3ChainAnalyzer

Specialized analyzer for X3 Chain workflows.

```python
analyzer = X3ChainAnalyzer()

# Analyze contracts
result = analyzer.analyze_rust_contract("crates/witness/src/lib.rs")

# Audit fraud proofs
result = analyzer.audit_fraud_proofs("crates/fraud_proofs/src/lib.rs")

# Validate configuration
result = analyzer.validate_x3_compliance("config/x3_chain_config.json")
```

## Usage Examples

### Example 1: Security Audit

```python
from x3_chain_ollama_tools import StreamingToolCaller

def check_unsafe_blocks(file_path: str) -> str:
    """Check for unsafe Rust blocks"""
    with open(file_path) as f:
        if "unsafe" in f.read():
            return f"⚠️  Unsafe blocks found in {file_path}"
    return f"✓ No unsafe blocks in {file_path}"

caller = StreamingToolCaller()
result = caller.call_single_tool(
    "Audit witness_v1.rs for unsafe code",
    tools=[check_unsafe_blocks],
    stream=True  # See output in real-time
)

if "unsafe" in result.final_response:
    print("Security Review Required!")
else:
    print("✓ Code passed security review")
```

### Example 2: Parallel Analysis

```python
def analyze_security(module: str) -> str:
    return f"Security: {module} - No vulnerabilities"

def check_performance(module: str) -> str:
    return f"Performance: {module} - Optimized loops detected"

def verify_tests(module: str) -> str:
    return f"Tests: {module} - 100% coverage"

result = caller.call_parallel_tools(
    "Audit the fraud_proofs module",
    tools=[analyze_security, check_performance, verify_tests],
    stream=True
)

print(result.final_response)
# Model will call all 3 tools in parallel and synthesize results
```

### Example 3: Interactive CLI Tool

```python
#!/usr/bin/env python3
import argparse
from x3_chain_ollama_tools import StreamingToolCaller

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--file", required=True)
    args = parser.parse_args()
    
    caller = StreamingToolCaller(
        render_thinking=True,   # Show reasoning
        render_content=True,    # Show response
        render_tool_calls=True, # Show tools
    )
    
    result = caller.call_single_tool(
        f"Audit {args.file}",
        tools=[your_analysis_function],
        stream=True  # Real-time output
    )
    
    print(f"\n✓ Analysis complete")

# Usage: python audit.py --file src/witness.rs
```

### Example 4: CI/CD Integration

```python
from x3_chain_ollama_tools import StreamingToolCaller

caller = StreamingToolCaller(
    render_content=False  # Don't spam CI logs
)

result = caller.call_single_tool(
    "Audit all Rust contracts",
    tools=[security_audit],
    stream=False  # Simple blocking for CI
)

# Fail CI if critical issues found
if "critical" in result.final_response.lower():
    print(f"❌ Audit failed: {result.final_response}")
    exit(1)

print("✓ Audit passed")
```

## Streaming vs Non-Streaming

| Feature | Streaming | Non-Streaming |
|---------|-----------|---------------|
| Real-time feedback | ✅ Yes | ❌ No |
| User experience | Better | Simpler |
| Best for | CLI, interactive | CI/CD, batch |
| Implementation | Slightly complex | Simple |
| Performance | ~ Same speed | ~ Same speed |

```python
# Streaming (user sees output immediately)
result = caller.call_single_tool(..., stream=True)

# Non-streaming (waits for complete response)
result = caller.call_single_tool(..., stream=False)
```

## Testing

```bash
# Run test suite
python test_x3_ollama_tools.py

# Expected output:
# ✓ PASS: Single Tool Call
# ✓ PASS: Parallel Tools
# ✓ PASS: Agent Loop
# ✓ PASS: X3 Chain Analyzer
# ✓ PASS: Streaming vs Non-Streaming
# ✓ PASS: ToolResult Structure  
# ✓ PASS: Error Handling
# Results: 7/7 tests passed
```

## Models You Can Use

| Model | Size | Best For | Thinking |
|-------|------|----------|----------|
| **qwen2.5-coder:7b** | 4.7GB | Balanced, recommended | ❌ No |
| **qwen3:latest** | ~8GB | Deep reasoning | ✅ Yes |
| **mistral:latest** | 4GB | Fast analysis | ❌ No |
| **neural-chat:7b** | 4.1GB | Lightweight | ❌ No |

Recommended: `qwen2.5-coder:7b` (default) or `qwen3` for thinking-capable models.

```python
# Use different models
caller = StreamingToolCaller(model="qwen3:latest")
```

## Performance

Typical execution times (GPU-accelerated):

- Single tool: 5-15 seconds
- Parallel tools (3): 10-20 seconds  
- Agent loop (5 iterations): 20-50 seconds

Non-streaming is 1-3 seconds faster, but difference is small.

## Architecture

```
User Code
    │
    ├─> StreamingToolCaller
    │       │
    │       ├─> call_single_tool()
    │       ├─> call_parallel_tools()
    │       └─> agent_loop()
    │
    ├─> Ollama API (localhost:11434)
    │       │
    │       └─> Model (qwen2.5-coder:7b, qwen3, etc)
    │
    └─> Returns ToolResult
            ├─ thinking (model reasoning)
            ├─ content (initial response)
            ├─ tool_calls_made
            └─ final_response
```

## Common Patterns for X3 Chain

### Security Audit Before Merge

```python
result = caller.call_parallel_tools(
    "Review PR changes for security",
    tools=[check_unsafe, check_overflow, verify_auth]
)

if "critical" in result.final_response:
    print("❌ Security issues found - block merge")
```

### Pre-Mainnet Validation

```python
result = caller.agent_loop(
    "Validate X3 Chain for mainnet deployment",
    tools=[
        check_consensus,
        verify_state_root,
        audit_fraud_proofs,
        validate_cross_vm,
    ],
    max_iterations=10
)
```

### Continuous Monitoring

```python
for component in components:
    result = caller.call_single_tool(
        f"Health check: {component}",
        tools=[check_health],
        stream=False  # Fast batch
    )
    if "critical" in result.final_response:
        alert(f"{component} needs attention")
```

## Troubleshooting

### "Module not found"

```bash
export PYTHONPATH=/home/lojak/Desktop/x3-chain-master:$PYTHONPATH
```

### "No tools called"

Ensure function has clear docstring:
```python
def analyze_code(file_path: str) -> str:
    """Analyze code for security issues"""  # ← Required!
    return "Analysis result"
```

### "Model doesn't support thinking"

Use qwen3 (has thinking) or set `think=False`:
```python
result = caller.call_single_tool(..., think=False)
```

See **X3_CHAIN_OLLAMA_DEPLOYMENT_GUIDE.py** for complete troubleshooting.

## Deployment Checklist

- [ ] Run `test_x3_ollama_tools.py` - all tests pass
- [ ] Try copy-paste examples from this README
- [ ] Add to your workflow (CLI tool, CI/CD, pre-commit hook)
- [ ] Monitor performance and accuracy
- [ ] Gather team feedback
- [ ] Iterate on tool definitions

See **X3_CHAIN_OLLAMA_DEPLOYMENT_GUIDE.py** for complete deployment guide.

## Next Steps

1. **Learn the patterns**: Read `STREAMING_PATTERNS_GUIDE.py`
2. **See real examples**: Check `X3_OLLAMA_USAGE_GUIDE.py`
3. **Run tests**: Execute `python test_x3_ollama_tools.py`
4. **Deploy**: Follow `X3_CHAIN_OLLAMA_DEPLOYMENT_GUIDE.py`
5. **Integrate**: Add to your codebase and workflows

## Documentation

- **API Reference**: See docstrings in `x3_chain_ollama_tools.py`
- **Patterns**: `STREAMING_PATTERNS_GUIDE.py` (6 complete examples)
- **Usage Guide**: `X3_OLLAMA_USAGE_GUIDE.py` (practical X3 examples)
- **Deployment**: `X3_CHAIN_OLLAMA_DEPLOYMENT_GUIDE.py` (production guide)
- **Testing**: `test_x3_ollama_tools.py` (test suite + manual examples)

## Support & Resources

- **Official Ollama Docs**: https://docs.ollama.com
- **Tool Calling API**: https://docs.ollama.com/capabilities/tool-calling.md
- **Streaming Guide**: https://docs.ollama.com/api/streaming.md
- **Known Issues**: Check project logs and troubleshooting guide

## License

Same as X3 Chain project

---

**Built for X3 Chain security auditing and AI-driven development**

Questions? Check the troubleshooting guide or test suite examples.

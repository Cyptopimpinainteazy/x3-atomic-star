#!/usr/bin/env python3
"""
X3 CHAIN OLLAMA STREAMING TOOLS - PRODUCTION DEPLOYMENT GUIDE

Complete guide for integrating streaming tool-calling into X3 Chain
development workflows and CI/CD pipelines.
"""

# ============================================================================
# QUICK START - COPY & PASTE READY CODE
# ============================================================================

COPY_PASTE_EXAMPLES = """
╔════════════════════════════════════════════════════════════════════════════╗
║                    COPY & PASTE READY EXAMPLES                            ║
╚════════════════════════════════════════════════════════════════════════════╝

EXAMPLE 1: Analyze a Single Rust File
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

from x3_chain_ollama_tools import StreamingToolCaller

def analyze_file(file_path: str) -> str:
    # Simulated analysis - replace with real implementation
    return f"✓ Analyzed {file_path}: No issues found"

caller = StreamingToolCaller()
result = caller.call_single_tool(
    f"Analyze {file_path} for security",
    tools=[analyze_file]
)
print(result.final_response)


EXAMPLE 2: Pre-Commit Hook - Block Unsafe Code
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#!/usr/bin/env python3
# Save as: .git/hooks/pre-commit

from x3_chain_ollama_tools import StreamingToolCaller
import sys

def security_check(file_path: str) -> str:
    if "unsafe" in open(file_path).read():
        return f"CRITICAL: Unsafe code in {file_path}"
    return f"OK: {file_path}"

# Check changed files
changed_files = ["crates/witness/src/lib.rs"]  # Get from git

caller = StreamingToolCaller(render_content=False)
for file in changed_files:
    result = caller.call_single_tool(
        f"Check {file}",
        tools=[security_check],
        stream=False
    )
    if "CRITICAL" in result.final_response:
        print(f"✗ Pre-commit check failed: {result.final_response}")
        sys.exit(1)

print("✓ All checks passed")
sys.exit(0)


EXAMPLE 3: CI/CD Pipeline Integration
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# In your GitHub Actions / CI pipeline:

name: Security Audit
on: [pull_request, push]

jobs:
  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-python@v4
      - run: pip install ollama
      
      - name: Run X3 Code Audit
        run: |
          python3 << 'EOF'
          from x3_chain_ollama_tools import StreamingToolCaller
          
          caller = StreamingToolCaller()
          result = caller.call_single_tool(
              "Audit all Rust code",
              tools=[lambda f: f"OK"],
              stream=False
          )
          
          with open("audit_report.txt", "w") as f:
              f.write(result.final_response)
          
          if "critical" in result.final_response.lower():
              exit(1)  # Fail CI
          EOF
      
      - name: Upload Audit Report
        uses: actions/upload-artifact@v3
        with:
          name: audit-report
          path: audit_report.txt


EXAMPLE 4: Batch Processing - Audit Multiple Files
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

from x3_chain_ollama_tools import StreamingToolCaller
import glob

def audit_file(path: str) -> str:
    return f"Audited {path}: OK"

caller = StreamingToolCaller(render_content=False)

files = glob.glob("crates/**/src/*.rs", recursive=True)
results = {}

for file in files:
    result = caller.call_single_tool(
        f"Audit {file}",
        tools=[audit_file],
        stream=False
    )
    results[file] = result.final_response

# Generate report
with open("batch_audit.txt", "w") as f:
    for file, response in results.items():
        f.write(f"{file}: {response}\\n")


EXAMPLE 5: Interactive CLI Tool
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#!/usr/bin/env python3
# Save as: tools/analyze.py

from x3_chain_ollama_tools import StreamingToolCaller
import argparse

def analyze_code(file_path: str) -> str:
    return f"Analyzed {file_path}: Ready for review"

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--file", required=True)
    args = parser.parse_args()
    
    caller = StreamingToolCaller(
        render_thinking=True,
        render_content=True,
        render_tool_calls=True,
    )
    
    result = caller.call_single_tool(
        f"Analyze {args.file}",
        tools=[analyze_code],
        stream=True  # Real-time output
    )
    
    print(f"\\n\\n=== ANALYSIS COMPLETE ===")
    print(f"Tools: {[t[0] for t in result.tool_calls_made]}")
    print(f"Result: {result.final_response}")

# Usage: python tools/analyze.py --file crates/witness/src/lib.rs


EXAMPLE 6: X3 Chain Compliance Check
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

from x3_chain_ollama_tools import X3ChainAnalyzer

analyzer = X3ChainAnalyzer()

# Quick compliance check
result = analyzer.validate_x3_compliance("config/x3_chain_config.json")

if "compliant" in result.final_response.lower():
    print("✓ Configuration is X3 compliant")
else:
    print("✗ Configuration issues found")
    print(result.final_response)
    exit(1)
"""

print(COPY_PASTE_EXAMPLES)


# ============================================================================
# DEPLOYMENT CHECKLIST
# ============================================================================

DEPLOYMENT_CHECKLIST = """
╔════════════════════════════════════════════════════════════════════════════╗
║                        DEPLOYMENT CHECKLIST                               ║
╚════════════════════════════════════════════════════════════════════════════╝

PHASE 1: LOCAL TESTING
━━━━━━━━━━━━━━━━━━━━━

☐ Install dependencies:
  pip install ollama

☐ Verify Ollama is running:
  ollama serve

☐ Download model:
  ollama pull qwen2.5-coder:7b

☐ Run test suite:
  python test_x3_ollama_tools.py

☐ Verify all tests pass:
  - ✓ Single Tool Call
  - ✓ Parallel Tools
  - ✓ Agent Loop
  - ✓ X3 Chain Analyzer
  - ✓ Streaming vs Non-Streaming
  - ✓ ToolResult Structure
  - ✓ Error Handling

☐ Test manually with your domain:
  python X3_OLLAMA_USAGE_GUIDE.py

☐ Verify streaming output renders correctly:
  - Can see thinking in real-time (if qwen3)
  - Can see tool calls as they appear
  - Can see content generation flow


PHASE 2: CI/CD INTEGRATION
━━━━━━━━━━━━━━━━━━━━━━━

☐ Create GitHub Actions workflow:
  .github/workflows/security-audit.yml

☐ Add to workflow:
  - Checkout code
  - Setup Python
  - Install ollama / qwen2.5-coder:7b
  - Run audit with x3_chain_ollama_tools
  - Block merge if critical issues found

☐ Add pre-commit hook:
  .git/hooks/pre-commit (or pre-commit framework)

☐ Update CI/CD timeout settings:
  - Single tools: 5-10 seconds
  - Parallel tools: 15-30 seconds
  - Agent loops: 60+ seconds

☐ Set up audit report storage:
  - Upload to artifacts
  - Save to S3/GCS
  - Include in merge commit message


PHASE 3: PROCESS INTEGRATION
━━━━━━━━━━━━━━━━━━━━━━━━

☐ Add to pull request template:
  "Audit results: [Run with x3_chain_ollama_tools]"

☐ Update developer documentation:
  - How to run local audits
  - How to interpret results
  - What to do if audit fails

☐ Create runbooks for:
  - Resolving common audit failures
  - Approving known safe warnings
  - Escalating critical issues

☐ Set up monitoring/alerting:
  - Alert on critical security issues
  - Track audit metrics over time
  - Monitor tool execution times


PHASE 4: PRODUCTION ROLLOUT
━━━━━━━━━━━━━━━━━━━━━━━━

☐ Deploy to staging environment:
  - Run against staging codebase
  - Verify audit results are accurate
  - Gather metrics on execution time

☐ Create runbook for operational support:
  - Troubleshooting guide
  - Escalation procedures
  - Performance tuning

☐ Train team on tool usage:
  - How to run audits locally
  - How to interpret results
  - When to use streaming vs non-streaming

☐ Add to onboarding documentation:
  - New developers learn tool early
  - Examples in codebase guide

☐ Monitor for issues:
  - Track audit failures
  - Monitor performance
  - Collect feedback


PHASE 5: CONTINUOUS IMPROVEMENT
━━━━━━━━━━━━━━━━━━━━━━━━━━━━

☐ Monthly review of:
  - Most common audit failures
  - False positives
  - Audit execution times
  - Model accuracy improvements

☐ Quarterly updates:
  - Try newer models (qwen3, etc)
  - Expand tool definitions
  - Add new analysis capabilities

☐ Collect feedback from:
  - Developers using the tools
  - Security reviewers
  - DevOps team

☐ Consider additional capabilities:
  - Vision model for architecture diagrams
  - Embedding searches for semantic analysis
  - Integration with other security tools
"""

print(DEPLOYMENT_CHECKLIST)


# ============================================================================
# PERFORMANCE CONSIDERATIONS
# ============================================================================

PERFORMANCE_GUIDE = """
╔════════════════════════════════════════════════════════════════════════════╗
║                    PERFORMANCE TUNING GUIDE                               ║
╚════════════════════════════════════════════════════════════════════════════╝

TIMING EXPECTATIONS
━━━━━━━━━━━━━━━━━━━

Model: qwen2.5-coder:7b (GPU-accelerated)

Single Tool Call:
  Stream=True:  5-15 seconds (real-time feedback)
  Stream=False: 3-8 seconds (blocking)

Parallel Tools (3 tools):
  Stream=True:  10-20 seconds (see tools as generated)
  Stream=False: 8-15 seconds (blocking)

Agent Loop (5 iterations):
  Stream=True:  20-50 seconds (watch reasoning)
  Stream=False: 15-40 seconds (blocking)

Note: Times depend on:
  - Prompt complexity
  - Model temperature
  - GPU availability
  - System load


OPTIMIZATION STRATEGIES
━━━━━━━━━━━━━━━━━━━━═

1. USE NON-STREAMING FOR CI/CD
   ────────────────────────────
   In pipelines, use stream=False:
   - Simpler output capture
   - Easier error handling
   - Slightly faster (no streaming overhead)
   
   result = caller.call_single_tool(..., stream=False)

2. BATCH PROCESSING
   ─────────────────
   Don't analyze one file at a time:
   
   # SLOW (sequential)
   for file in files:
       result = caller.call_single_tool(...)
   
   # FAST (parallel)
   result = caller.call_parallel_tools(...)

3. USE STREAMING FOR CLI
   ──────────────────────
   In interactive tools, use stream=True:
   - User sees immediate feedback
   - Better perceived performance
   - More engaging UX
   
   result = caller.call_single_tool(..., stream=True)

4. REDUCE MODEL ITERATIONS
   ────────────────────────
   For agent loops, set reasonable max:
   
   # OK: Simple math
   result = caller.agent_loop(..., max_iterations=3)
   
   # RISKY: Could timeout
   result = caller.agent_loop(..., max_iterations=20)

5. OPTIMIZE TOOL DEFINITIONS
   ────────────────────────────
   Keep tools fast:
   
   # SLOW: File I/O in tool
   def analyze(path: str) -> str:
       content = open(path).read()  # Slow
       return analyze_content(content)
   
   # FAST: Return quick analysis metadata
   def analyze(path: str) -> str:
       return "File size: 1024 bytes, Lines: 42"

6. USE RENDER SELECTIVELY
   ───────────────────────
   In CI/CD, disable rendering:
   
   caller = StreamingToolCaller(
       render_thinking=False,  # Disable in CI
       render_content=False,   # Don't spam logs
       render_tool_calls=False,
   )

7. CACHE RESULTS
   ──────────────
   For repeated analysis:
   
   # Check cache first
   if cache_hit:
       return cached_result
   
   # Otherwise run analysis
   result = caller.call_single_tool(...)
   cache[file_path] = result
   return result


MONITORING & METRICS
━━━━━━━━━━━━━━━━━━

Track these metrics to identify bottlenecks:

1. Execution Time
   - Single tool: Target < 10s
   - Parallel: Target < 20s
   - Agent loop: Historical average

2. Tool Call Success Rate
   - % of requests that generate tool calls
   - % of successful tool executions

3. Error Rate
   - Parsing failures
   - Timeout percentage
   - Model crashes

4. User Experience
   - Streaming enabled: % who see real-time output
   - CI/CD blocking: % of failed audits
   - False positive: % of warnings ignored

Store metrics:
- In logs with timestamps
- In monitoring system (Datadog, New Relic, etc)
- Dashboard for team visibility


SCALING TO MULTIPLE MODELS
━━━━━━━━━━━━━━━━━━━━━━━

Try multiple models and compare:

from x3_chain_ollama_tools import StreamingToolCaller

models = [
    "qwen2.5-coder:7b",      # Default
    "qwen3:latest",          # With thinking
    "mistral:latest",        # Alternative
    "neural-chat:latest",    # Lightweight
]

for model in models:
    caller = StreamingToolCaller(model=model)
    result = caller.call_single_tool(...)
    print(f"{model}: {result.final_response}")
    # Compare quality and speed

Document findings:
- Which model best for each task
- Speed vs quality tradeoffs
- Resource requirements
"""

print(PERFORMANCE_GUIDE)


# ============================================================================
# TROUBLESHOOTING
# ============================================================================

TROUBLESHOOTING = """
╔════════════════════════════════════════════════════════════════════════════╗
║                       TROUBLESHOOTING GUIDE                               ║
╚════════════════════════════════════════════════════════════════════════════╝

ISSUE: "Module not found: x3_chain_ollama_tools"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Solution:
  1. Verify file exists in workspace:
     ls -la x3_chain_ollama_tools.py
  
  2. Ensure Python path is correct:
     export PYTHONPATH=/home/lojak/Desktop/x3-chain-master:$PYTHONPATH
  
  3. Or run from correct directory:
     cd /home/lojak/Desktop/x3-chain-master
     python3 your_script.py


ISSUE: "No tools called" / "Tool calls empty"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Cause: Model isn't recognizing tools in prompt

Solution:
  1. Verify tools list is passed:
     result = caller.call_single_tool(..., tools=[your_function])
  
  2. Check function docstring:
     def your_function(arg: str) -> str:
         """Clear description required for model to understand"""
     
  3. Try simpler prompt:
     caller.call_single_tool("Analyze file", tools=[...])
     # Instead of: "Perform deep architectural analysis..."
  
  4. Use longer model time:
     # Model might be thinking, not calling tools yet


ISSUE: "Model doesn't support thinking"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Solution:
  1. Use qwen3 (has thinking):
     caller = StreamingToolCaller(model="qwen3:latest")
  
  2. Or disable thinking:
     result = caller.call_single_tool(..., think=False)
  
  3. Check model supports feature:
     # Not all models support all features


ISSUE: "API timeout / slow responses"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Solution:
  1. Check Ollama is running:
     ollama serve
  
  2. Try non-streaming (might be faster):
     stream=False
  
  3. Reduce agent loop iterations:
     max_iterations=3  # Instead of 10
  
  4. Simplify tools:
     # Fewer, faster tools
  
  5. Check system resources:
     top  # Monitor GPU/CPU
     nvidia-smi  # Check GPU


ISSUE: "Streaming output not rendering"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Solution:
  1. Enable rendering:
     caller = StreamingToolCaller(render_content=True)
  
  2. Check stream=True is set:
     result = caller.call_single_tool(..., stream=True)
  
  3. Verify stdout is not buffered:
     python -u script.py  # Unbuffered mode
  
  4. Check terminal:
     # Make sure terminal supports output


ISSUE: "ToolResult missing fields"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Solution:
  1. Verify function signature:
     def get_tool_name() -> str or list or dict
  
  2. Check return type:
     # Must return JSON-serializable
  
  3. Update wrapper if needed:
     # File x3_chain_ollama_tools.py


ISSUE: "Tool execution errors"
━━━━━━━━━━━━━━━━━━━━━━━━━━

Solution:
  1. Verify tool function works standalone:
     result = your_tool("test_arg")
     print(result)
  
  2. Check argument types:
     # Function expects str, model passing dict?
  
  3. Add error handling to tool:
     def safe_tool(arg):
         try:
             return do_thing(arg)
         except Exception as e:
             return f"Error: {e}"


DEBUG MODE
━━━━━━━━━

Enable full debugging:

caller = StreamingToolCaller(
    model="qwen2.5-coder:7b",
    render_thinking=True,      # Show thinking
    render_content=True,       # Show content
    render_tool_calls=True,    # Show tool calls
)

# Then manually check each field:
result = caller.call_single_tool(...)
print(f"Thinking: {result.thinking}")
print(f"Content: {result.content}")
print(f"Tools: {result.tool_calls_made}")
print(f"Final: {result.final_response}")

If stuck, check:
  1. Ollama logs: ~/.ollama/logs
  2. Tool definitions have docstrings
  3. Model is loaded: ollama list
  4. Enough disk space for models
"""

print(TROUBLESHOOTING)


# ============================================================================
# FILE MANIFEST
# ============================================================================

MANIFEST = """
╔════════════════════════════════════════════════════════════════════════════╗
║                    PROJECT FILE MANIFEST                                  ║
╚════════════════════════════════════════════════════════════════════════════╝

Core Implementation Files
━━━━━━━━━━━━━━━━━━━━━━━

x3_chain_ollama_tools.py
  ├─ StreamingToolCaller (main wrapper)
  │  ├─ call_single_tool()
  │  ├─ call_parallel_tools()
  │  └─ agent_loop()
  ├─ X3ChainAnalyzer (specialized for X3)
  │  ├─ analyze_rust_contract()
  │  ├─ audit_fraud_proofs()
  │  └─ validate_x3_compliance()
  └─ ToolResult dataclass

Documentation Files
━━━━━━━━━━━━━━━━━

STREAMING_PATTERNS_GUIDE.py
  └─ 6 complete patterns (non-streaming & streaming examples)
     - Single tool calls
     - Parallel tools
     - Agent loops
     - Comparison matrix

X3_OLLAMA_USAGE_GUIDE.py
  └─ Practical examples for X3 Chain
     - Contract analysis
     - Fraud proofs audit
     - Configuration validation
     - Cross-VM verification

X3_CHAIN_OLLAMA_DEPLOYMENT_GUIDE.py (this file)
  └─ Production deployment guide
     - Copy & paste ready examples
     - Deployment checklist
     - Performance tuning
     - Troubleshooting

Test File
━━━━━━━

test_x3_ollama_tools.py
  └─ 7 automated tests
     - Single tool calling
     - Parallel tools
     - Agent loop
     - X3 analyzer
     - Streaming vs non-streaming
     - Error handling

Documentation Index
━━━━━━━━━━━━━━━━━

OLLAMA_TOOL_CALLING_RESULTS.md
  └─ Analysis of qwen3:1.7b vs qwen2.5-coder:7b
     - Why larger models work better
     - Test results
     - Recommendations


USAGE SUMMARY
━━━━━━━━━━━━

To use the tools:

1. Import:
   from x3_chain_ollama_tools import StreamingToolCaller

2. Create caller:
   caller = StreamingToolCaller()

3. Call tools:
   result = caller.call_single_tool(..., tools=[...])

4. Process result:
   print(result.final_response)

5. Access details:
   - result.thinking: Model reasoning
   - result.content: Initial response
   - result.tool_calls_made: Which tools were called
   - result.final_response: Synthesized answer
"""

print(MANIFEST)


if __name__ == "__main__":
    print("\n" + "="*80)
    print("X3 CHAIN OLLAMA STREAMING TOOLS - DEPLOYMENT GUIDE")
    print("="*80)
    print("\nAll guides printed above.")
    print("\nNext steps:")
    print("  1. Review checklists")
    print("  2. Run: python test_x3_ollama_tools.py")
    print("  3. Try a copy-paste example")
    print("  4. Integrate into your workflow")

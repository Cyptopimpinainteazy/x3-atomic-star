#!/usr/bin/env python3
"""
X3 Chain Ollama Tools: Quick Reference & Usage Guide

A complete guide for X3 Chain developers to use streaming tool-calling
for code analysis, security audits, and validation workflows.
"""

# ============================================================================
# QUICK START - 3 LINES TO GET STARTED
# ============================================================================

from x3_chain_ollama_tools import StreamingToolCaller, X3ChainAnalyzer

# Initialize the tool caller
caller = StreamingToolCaller(model="qwen2.5-coder:7b")

# Call tools with real-time streaming
result = caller.call_single_tool(
    "Analyze witness_v1.rs for security",
    tools=[lambda fp: f"Security review of {fp}: OK"]
)

print(f"Result: {result.final_response}")


# ============================================================================
# EXAMPLE 1: ANALYZE RUST SMART CONTRACTS
# ============================================================================

def example_analyze_contract():
    """Analyze a smart contract for vulnerabilities."""
    
    def analyze_arithmetic_safety(file_path: str) -> str:
        """Check for arithmetic overflow/underflow issues"""
        return f"✓ {file_path}: No unsafe arithmetic operations detected"
    
    def check_state_access(file_path: str) -> str:
        """Verify proper state access controls"""
        return f"✓ {file_path}: All state access properly guarded"
    
    def verify_cross_vm_safety(file_path: str) -> str:
        """Verify cross-VM calls are safe"""
        return f"✓ {file_path}: Cross-VM invariants maintained"
    
    caller = StreamingToolCaller()
    result = caller.call_parallel_tools(
        f"Perform comprehensive security audit of contracts/fraud_proofs.rs",
        tools=[
            analyze_arithmetic_safety,
            check_state_access,
            verify_cross_vm_safety,
        ]
    )
    
    print("\n=== Smart Contract Security Audit ===")
    print(f"Analysis: {result.content}")
    print(f"Tools Executed: {[t[0] for t in result.tool_calls_made]}")
    print(f"\n{result.final_response}")


# ============================================================================
# EXAMPLE 2: USE THE X3 CHAIN ANALYZER
# ============================================================================

def example_x3_analyzer():
    """Use the specialized X3 Chain analyzer."""
    
    analyzer = X3ChainAnalyzer(model="qwen2.5-coder:7b")
    
    # Analyze fraud proofs
    print("\n=== Fraud Proofs Audit ===")
    result = analyzer.audit_fraud_proofs("crates/fraud_proofs/src/lib.rs")
    print(f"Tools Called: {[t[0] for t in result.tool_calls_made]}")
    print(f"Final Report: {result.final_response}")
    
    # Validate configuration
    print("\n=== X3 Chain Compliance Check ===")
    result = analyzer.validate_x3_compliance("config/x3_chain_config.json")
    print(f"Tools Called: {[t[0] for t in result.tool_calls_made]}")
    print(f"Final Report: {result.final_response}")


# ============================================================================
# EXAMPLE 3: AGENT LOOP FOR COMPLEX REASONING
# ============================================================================

def example_agent_loop():
    """Use agent loop for multi-step analysis."""
    
    # Define analysis tools
    def parse_ast(file_path: str) -> str:
        """Parse code AST and extract structure"""
        return f"Parsed {file_path}: 42 functions, 8 structs, 156 traits"
    
    def check_invariants(file_path: str) -> str:
        """Check code invariants"""
        return f"Invariants in {file_path}: All maintained, no violations"
    
    def generate_report(analysis: str) -> str:
        """Generate analysis report"""
        return f"Report generated with findings: {analysis}"
    
    caller = StreamingToolCaller()
    result = caller.agent_loop(
        "Perform deep architectural analysis of crates/core/src/lib.rs",
        tools=[parse_ast, check_invariants, generate_report],
        max_iterations=5
    )
    
    print("\n=== Deep Architecture Analysis ===")
    print(f"Iterations: {len(result.tool_calls_made)}")
    print(f"Final Analysis: {result.final_response}")


# ============================================================================
# EXAMPLE 4: CUSTOM WORKFLOW FOR CROSS-VM VALIDATION
# ============================================================================

def example_cross_vm_validation():
    """Validate cross-VM compatibility."""
    
    def verify_evm_bridge(config: str) -> str:
        """Verify EVM bridge configuration"""
        return "✓ EVM Bridge: Properly configured, all contracts deployed"
    
    def verify_svm_integration(config: str) -> str:
        """Verify SVM integration"""
        return "✓ SVM: PDAs correct, instruction routing valid"
    
    def check_state_sync(config: str) -> str:
        """Verify state synchronization"""
        return "✓ State Sync: Merkle proofs valid, updates atomic"
    
    def validate_consensus(config: str) -> str:
        """Validate consensus parameters"""
        return "✓ Consensus: Block time optimal, finality correct"
    
    caller = StreamingToolCaller()
    result = caller.call_parallel_tools(
        "Validate cross-VM integration for mainnet deployment",
        tools=[
            verify_evm_bridge,
            verify_svm_integration,
            check_state_sync,
            validate_consensus,
        ]
    )
    
    print("\n=== Cross-VM Validation Report ===")
    print(f"Status: {'PASSED' if all(t[0] in result.final_response for t in result.tool_calls_made) else 'FAILED'}")
    for tool_name, args in result.tool_calls_made:
        print(f"  ✓ {tool_name}({list(args.values())[0]})")
    print(f"\nValidation Summary:\n{result.final_response}")


# ============================================================================
# EXAMPLE 5: BATCH ANALYSIS - MULTIPLE FILES
# ============================================================================

def example_batch_analysis():
    """Analyze multiple files in a batch."""
    
    files_to_audit = [
        "crates/witness/src/witness_v1.rs",
        "crates/fraud_proofs/src/lib.rs",
        "crates/state_root/src/lib.rs",
    ]
    
    def audit_file(file_path: str) -> str:
        """Generic file audit"""
        return f"Audited {file_path}: 0 critical, 2 warnings, 4 info"
    
    caller = StreamingToolCaller()
    
    print("\n=== Batch File Audit ===")
    for file_path in files_to_audit:
        result = caller.call_single_tool(
            f"Audit {file_path}",
            tools=[audit_file],
            stream=False  # Non-streaming for batch
        )
        print(f"{file_path}: {result.final_response}")


# ============================================================================
# EXAMPLE 6: STREAMING WITH THINKING (For Capable Models)
# ============================================================================

def example_with_thinking():
    """Use thinking capability for deeper analysis."""
    
    def deep_analysis(component: str) -> str:
        """Perform deep code analysis"""
        return f"Deep analysis of {component} complete"
    
    caller = StreamingToolCaller(
        model="qwen3:latest",  # Uses thinking-capable model if available
        render_thinking=True,   # Show thinking process
        render_content=True,    # Show content
    )
    
    result = caller.call_single_tool(
        "Why might duplicate code in fraud_proofs module be problematic?",
        tools=[deep_analysis],
    )
    
    print("\n=== Analysis with Reasoning ===")
    if result.thinking:
        print(f"Model Reasoning:\n{result.thinking}\n")
    print(f"Final Answer:\n{result.final_response}")


# ============================================================================
# REAL WORLD SCENARIOS
# ============================================================================

WORKFLOWS = """
╔════════════════════════════════════════════════════════════════════════════╗
║                   X3 CHAIN REAL WORKFLOWS                                 ║
╚════════════════════════════════════════════════════════════════════════════╝

SCENARIO 1: Code Review Before Merge
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  from x3_chain_ollama_tools import StreamingToolCaller
  
  caller = StreamingToolCaller()
  
  # During PR review - analyze changed files
  result = caller.call_parallel_tools(
      "Review changes in crates/witness/src/witness_v1.rs",
      tools=[
          lambda f: f"Security: OK",
          lambda f: f"Performance: Good",
          lambda f: f"Tests: Complete",
      ]
  )
  
  print(result.final_response)  # Display in PR comment


SCENARIO 2: Pre-Mainnet Security Audit
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  from x3_chain_ollama_tools import X3ChainAnalyzer
  
  analyzer = X3ChainAnalyzer()
  
  # Full mainnet audit
  results = {
      "fraud_proofs": analyzer.audit_fraud_proofs("..."),
      "validator": analyzer.validate_x3_compliance("..."),
      "contracts": analyzer.analyze_rust_contract("..."),
  }
  
  # Combine results for audit report
  for component, result in results.items():
      print(f"{component}: {result.final_response}")


SCENARIO 3: Continuous Monitoring (Agent Loop)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  from x3_chain_ollama_tools import StreamingToolCaller
  
  caller = StreamingToolCaller()
  
  # Monitor for issues - agent decides what to check
  result = caller.agent_loop(
      "Check health of X3 Chain deployment",
      tools=[
          check_block_production,
          verify_finality,
          audit_state_root,
          check_validator_set,
      ],
      max_iterations=10
  )
  # Model will decide when enough checks are done


SCENARIO 4: Interactive DevOps Tool
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  from x3_chain_ollama_tools import StreamingToolCaller
  
  caller = StreamingToolCaller(
      render_thinking=True,
      render_content=True,
      render_tool_calls=True,
  )
  
  # User runs: python analyze.py --file witness_v1.rs
  result = caller.call_parallel_tools(
      f"Analyze {args.file}",
      tools=[security_check, perf_check, test_coverage],
      stream=True  # Real-time output
  )
  
  # User sees streaming analysis in CLI


SCENARIO 5: Automated CI/CD Analysis
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  from x3_chain_ollama_tools import StreamingToolCaller
  
  caller = StreamingToolCaller()
  
  # Run in CI/CD pipeline - non-streaming for simplicity
  result = caller.call_parallel_tools(..., stream=False)
  
  if "critical" in result.final_response.lower():
      exit(1)  # Fail CI on critical issues
  
  # Log results
  with open("audit_report.txt", "w") as f:
      f.write(result.final_response)


KEY INTEGRATION POINTS
━━━━━━━━━━━━━━━━━━━━
  
  1. Import the wrapper:
     from x3_chain_ollama_tools import StreamingToolCaller, X3ChainAnalyzer
  
  2. Create a caller (once per session):
     caller = StreamingToolCaller(model="qwen2.5-coder:7b")
  
  3. Choose your pattern:
     • Single tool: call_single_tool()
     • Parallel: call_parallel_tools()
     • Complex: agent_loop()
  
  4. Define analysis functions matching your domain
  
  5. Process results:
     - result.thinking: Model's reasoning (if available)
     - result.content: Initial response
     - result.tool_calls_made: Which tools were called
     - result.final_response: Final synthesized answer


STREAMING vs NON-STREAMING CHOICE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  
  stream=True (default):
    ✓ Use for: CLI tools, user-facing analysis, long responses
    ✓ Shows: Real-time output, thinking progress, tool calls
    ✗ Code: Slightly more complex (but wrapper handles it)
  
  stream=False:
    ✓ Use for: CI/CD, batch processing, simple workflows
    ✓ Advantage: Simpler to debug
    ✗ Downside: User sees nothing until done


DEFAULT PARAMETERS
━━━━━━━━━━━━━━━━━
  
  caller = StreamingToolCaller(
      model="qwen2.5-coder:7b",  # Change to qwen3 for thinking
      render_thinking=True,       # Show model reasoning
      render_content=True,        # Show response
      render_tool_calls=True,     # Show tools being called
  )
"""

print(WORKFLOWS)


# ============================================================================
# API REFERENCE
# ============================================================================

API_REFERENCE = """
╔════════════════════════════════════════════════════════════════════════════╗
║                        API REFERENCE                                       ║
╚════════════════════════════════════════════════════════════════════════════╝

class StreamingToolCaller:
    '''Streaming tool-calling wrapper for Ollama models.'''
    
    def __init__(model, render_thinking, render_content, render_tool_calls)
        '''Initialize with model and rendering options.'''
    
    def call_single_tool(user_message, tools, stream=True, think=True) → ToolResult
        '''Execute single tool with streaming.
        
        Args:
            user_message: What to ask
            tools: List of callable functions
            stream: Enable streaming (default True)
            think: Enable thinking (default True)
        
        Returns:
            ToolResult with thinking, content, tool_calls, final_response
        '''
    
    def call_parallel_tools(user_message, tools, stream=True, think=True) → ToolResult
        '''Execute multiple tools in parallel.'''
    
    def agent_loop(user_message, tools, max_iterations=10, stream=True, think=True) → ToolResult
        '''Multi-turn loop - model decides when to call tools.'''


class X3ChainAnalyzer:
    '''Specialized analyzer for X3 Chain code.'''
    
    def analyze_rust_contract(file_path) → ToolResult
        '''Security & performance audit of Rust contract.'''
    
    def audit_fraud_proofs(module_path) → ToolResult
        '''Audit fraud proofs module for correctness.'''
    
    def validate_x3_compliance(config_path) → ToolResult
        '''Validate X3 configuration compliance.'''


@dataclass ToolResult:
    thinking: str              # Model's reasoning
    content: str               # Initial response
    tool_calls_made: list      # [(tool_name, args), ...]
    final_response: str        # Final synthesized answer
"""

print(API_REFERENCE)


# ============================================================================
# Run examples
# ============================================================================

if __name__ == "__main__":
    print("\n" + "="*80)
    print("X3 CHAIN OLLAMA STREAMING TOOLS - USAGE GUIDE")
    print("="*80 + "\n")
    
    print("To run examples, uncomment the example functions below:\n")
    print("  example_analyze_contract()")
    print("  example_x3_analyzer()")
    print("  example_agent_loop()")
    print("  example_cross_vm_validation()")
    print("  example_batch_analysis()")
    print("  example_with_thinking()")
    print("\nOr copy patterns directly into your code!")

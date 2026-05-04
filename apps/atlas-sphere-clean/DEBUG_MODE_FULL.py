#!/usr/bin/env python3
"""
X3 Chain Ollama Tools - Full Debug Mode

Implements all debugging techniques:
1. Enable all rendering (thinking, content, tool calls)
2. Show what tools were called
3. Display thinking process
4. Show final synthesis
5. Test with qwen3 (thinking-capable model)
"""

from x3_chain_ollama_tools import StreamingToolCaller, ToolResult


def debug_single_tool_with_full_rendering():
    """Debug mode: Single tool call with all rendering enabled."""
    print("\n" + "="*80)
    print("TEST 1: Single Tool Call - Full Debug Rendering")
    print("="*80)
    
    def analyze_code(file_path: str) -> str:
        """Analyze code for vulnerabilities"""
        return f"Analysis of {file_path}: 0 critical, 2 warnings, security OK"
    
    # TIP 1: Enable all rendering
    caller = StreamingToolCaller(
        model="qwen2.5-coder:7b",
        render_thinking=True,
        render_content=True,
        render_tool_calls=True,
    )
    
    print("\n[Step 1] Calling with all rendering enabled...")
    result = caller.call_single_tool(
        "Analyze witness_v1.rs for security issues",
        tools=[analyze_code],
        stream=False,
    )
    
    # TIP 2: Check what tools were called
    print("\n[Step 2] Tools that were called:")
    tools_executed = [t[0] for t in result.tool_calls_made]
    print(f"  {tools_executed}")
    print(f"  Total tools: {len(result.tool_calls_made)}")
    if result.tool_calls_made:
        for tool_name, args in result.tool_calls_made:
            print(f"    - {tool_name}({args})")
    
    # TIP 3: See the thinking process
    print("\n[Step 3] Model Thinking Process:")
    if result.thinking:
        print(f"  {result.thinking}")
    else:
        print("  (No thinking output - model doesn't support thinking or didn't use it)")
    
    # TIP 4: Check final synthesis
    print("\n[Step 4] Final Synthesis:")
    print(f"  {result.final_response}")
    
    print("\n✓ Test complete")


def test_with_thinking_capable_model():
    """TEST: Test with qwen3 (thinking-capable model)."""
    print("\n" + "="*80)
    print("TEST 2: With Thinking-Capable Model (qwen3)")
    print("="*80)
    
    def verify_security(module: str) -> str:
        """Verify module security"""
        return f"Verified {module}: All checks passed"
    
    # TIP 5: Test with qwen3
    print("\n[Checking available models...]")
    
    try:
        # Try to use qwen3 with thinking enabled
        caller = StreamingToolCaller(
            model="qwen3:latest",  # Thinking-capable model
            render_thinking=True,
            render_content=True,
            render_tool_calls=True,
        )
        
        print("✓ qwen3 model available")
        
        result = caller.call_single_tool(
            "Verify fraud_proofs module security",
            tools=[verify_security],
            stream=False,
        )
        
        print("\n[Thinking Output]:")
        if result.thinking:
            print(f"  {result.thinking}")
        else:
            print("  (Model generated no thinking)")
        
        print("\n[Final Response]:")
        print(f"  {result.final_response}")
        
    except Exception as e:
        print(f"⚠️  qwen3 not available: {e}")
        print("  Falling back to qwen2.5-coder:7b (non-thinking model)")
        
        caller = StreamingToolCaller(
            model="qwen2.5-coder:7b",
            render_thinking=False,  # This model doesn't support it
            render_content=True,
            render_tool_calls=True,
        )
        
        result = caller.call_single_tool(
            "Quick security check",
            tools=[verify_security],
            stream=False,
        )
        print(f"\n✓ Result: {result.final_response}")


def debug_parallel_tools():
    """Debug: Parallel tools with full insights."""
    print("\n" + "="*80)
    print("TEST 3: Parallel Tools - Full Debug Output")
    print("="*80)
    
    def check_arithmetic(component: str) -> str:
        return f"✓ {component}: No overflow/underflow risks"
    
    def verify_access_control(component: str) -> str:
        return f"✓ {component}: Access control verified"
    
    def validate_state(component: str) -> str:
        return f"✓ {component}: State consistency OK"
    
    caller = StreamingToolCaller(
        render_thinking=True,
        render_content=True,
        render_tool_calls=True,
    )
    
    print("\n[Parallel tool execution with full debug]")
    result = caller.call_parallel_tools(
        "Audit fraud_proofs module for security and correctness",
        tools=[check_arithmetic, verify_access_control, validate_state],
        stream=False,
    )
    
    # Debug info
    print("\n[Tools Executed]:")
    for tool_name, args in result.tool_calls_made:
        print(f"  • {tool_name}({list(args.values())[0] if args else ''})")
    
    print(f"\n[Execution Count]: {len(result.tool_calls_made)} tools")
    
    print("\n[Final Report]:")
    print(result.final_response)


def debug_agent_loop():
    """Debug: Agent loop with step-by-step output."""
    print("\n" + "="*80)
    print("TEST 4: Agent Loop - Step-by-Step Debug Output")
    print("="*80)
    
    def add(a: int, b: int) -> int:
        """Add two numbers"""
        result = a + b
        print(f"    [Tool Result] {a} + {b} = {result}")
        return result
    
    def multiply(a: int, b: int) -> int:
        """Multiply two numbers"""
        result = a * b
        print(f"    [Tool Result] {a} * {b} = {result}")
        return result
    
    caller = StreamingToolCaller(
        render_thinking=True,
        render_content=True,
        render_tool_calls=True,
    )
    
    print("\nSolving: What is (5 + 3) * 2?")
    print("(Expected: 5 + 3 = 8, then 8 * 2 = 16)")
    print()
    
    result = caller.agent_loop(
        "Calculate (5+3)*2",
        tools=[add, multiply],
        max_iterations=5,
        stream=False,
    )
    
    # Debug output
    print("\n[Agent Loop Analysis]:")
    print(f"  Total steps: {len(result.tool_calls_made)}")
    print(f"  Tools used: {[t[0] for t in result.tool_calls_made]}")
    print(f"  Final answer: {result.final_response}")


def comprehensive_debug_comparison():
    """Comprehensive comparison: Streaming vs Non-Streaming with debug output."""
    print("\n" + "="*80)
    print("TEST 5: Streaming vs Non-Streaming Comparison")
    print("="*80)
    
    def quick_analysis(file: str) -> str:
        return f"Analysis of {file}: PASS"
    
    # Non-streaming with debug
    print("\n[Non-Streaming Mode]")
    print("-" * 40)
    
    caller = StreamingToolCaller(
        render_thinking=False,
        render_content=True,
        render_tool_calls=True,
    )
    
    result_nonstream = caller.call_single_tool(
        "Analyze code",
        tools=[quick_analysis],
        stream=False,
    )
    
    print(f"Tools called: {[t[0] for t in result_nonstream.tool_calls_made]}")
    print(f"Result available: {bool(result_nonstream.final_response)}")
    
    # Streaming with debug
    print("\n[Streaming Mode]")
    print("-" * 40)
    
    result_stream = caller.call_single_tool(
        "Analyze code",
        tools=[quick_analysis],
        stream=True,
    )
    
    print(f"Tools called: {[t[0] for t in result_stream.tool_calls_made]}")
    print(f"Result available: {bool(result_stream.final_response)}")
    
    print("\n✓ Both modes completed successfully")


def print_debug_tips():
    """Print all 5 debugging tips."""
    tips = """
╔════════════════════════════════════════════════════════════════════════════╗
║                        5 DEBUGGING TIPS FOR X3 TOOLS                      ║
╚════════════════════════════════════════════════════════════════════════════╝

TIP 1: Enable All Rendering
━━━━━━━━━━━━━━━━━━━━━━━━━━━

caller = StreamingToolCaller(
    render_thinking=True,      # Show model thinking
    render_content=True,       # Show responses
    render_tool_calls=True,    # Show tools as called
)

Result: You see everything the model is doing in real-time!


TIP 2: Check What Tools Were Called
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

result = caller.call_single_tool(...)

# See which tools were invoked
tools_used = [t[0] for t in result.tool_calls_made]
print(tools_used)
# Output: ['analyze_rust_code', 'check_performance']

# See the arguments
for tool_name, args in result.tool_calls_made:
    print(f"{tool_name}({args})")


TIP 3: See the Thinking Process
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# Only works with thinking-capable models (qwen3)
print(result.thinking)

# Example output:
# "First I need to understand the code structure...
#  Then check for security vulnerabilities...
#  Let me verify the arithmetic safety..."


TIP 4: Check Final Synthesis
━━━━━━━━━━━━━━━━━━━━━━━━━━

print(result.final_response)

# This is what the model concluded after:
# 1. Thinking (if available)
# 2. Calling tools
# 3. Processing results
# 4. Synthesizing a final answer


TIP 5: Test With Thinking-Capable Models
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

# qwen3 has thinking capability
caller = StreamingToolCaller(model="qwen3:latest")

# Enable thinking rendering to see the process
caller = StreamingToolCaller(
    model="qwen3:latest",
    render_thinking=True,
)

# Compare thinking output to tool outputs
result = caller.call_single_tool(...)
print(f"Thinking: {result.thinking}")
print(f"Tools: {result.tool_calls_made}")
print(f"Conclusion: {result.final_response}")


PRACTICAL WORKFLOW
━━━━━━━━━━━━━━━━━

1. Start with non-streaming + minimal rendering:
   caller = StreamingToolCaller()
   result = caller.call_single_tool(..., stream=False)

2. If tools aren't being called:
   Enable all rendering to see why
   caller = StreamingToolCaller(render_thinking=True, render_content=True)

3. If you need deeper reasoning:
   Switch to qwen3 model (thinking-capable)
   caller = StreamingToolCaller(model="qwen3:latest", render_thinking=True)

4. Once working, optimize:
   - Use streaming=True for interactive tools
   - Use streaming=False for CI/CD
   - Disable rendering for cleaner output
"""
    print(tips)


if __name__ == "__main__":
    print("\n" + "="*80)
    print("X3 CHAIN OLLAMA TOOLS - FULL DEBUG MODE")
    print("="*80)
    
    print_debug_tips()
    
    print("\n" + "="*80)
    print("RUNNING DEBUG TESTS")
    print("="*80)
    
    try:
        debug_single_tool_with_full_rendering()
    except Exception as e:
        print(f"✗ Test 1 failed: {e}")
        import traceback
        traceback.print_exc()
    
    try:
        debug_parallel_tools()
    except Exception as e:
        print(f"✗ Test 3 failed: {e}")
    
    try:
        debug_agent_loop()
    except Exception as e:
        print(f"✗ Test 4 failed: {e}")
    
    try:
        test_with_thinking_capable_model()
    except Exception as e:
        print(f"✗ Test 2 failed: {e}")
    
    try:
        comprehensive_debug_comparison()
    except Exception as e:
        print(f"✗ Test 5 failed: {e}")
    
    print("\n" + "="*80)
    print("DEBUG TESTS COMPLETE")
    print("="*80)
    print("\nUse these debugging techniques in your own code!")

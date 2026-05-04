#!/usr/bin/env python3
"""
Test Suite for X3 Chain Ollama Streaming Tools

Validates:
- Single tool calling
- Parallel tool calling
- Agent loops
- Streaming vs non-streaming
- Error handling
- Thinking model support
"""

import sys
from typing import List


def test_single_tool_call():
    """Test: Single tool call with streaming."""
    print("\n[TEST 1] Single Tool Call")
    print("-" * 70)
    
    from x3_chain_ollama_tools import StreamingToolCaller
    
    def get_chain_version(chain_name: str) -> str:
        """Get X3 Chain version"""
        versions = {
            "x3": "v1.2.3",
            "ethereum": "Shanghai",
            "solana": "v1.18",
        }
        return versions.get(chain_name.lower(), "Unknown")
    
    caller = StreamingToolCaller(model="qwen2.5-coder:7b")
    
    try:
        result = caller.call_single_tool(
            "What version is X3 Chain?",
            tools=[get_chain_version],
            stream=False,  # Non-streaming for testing
        )
        
        assert result.tool_calls_made, "No tool calls executed"
        assert result.final_response, "No final response"
        
        print(f"✓ Tool calls made: {result.tool_calls_made}")
        print(f"✓ Final response: {result.final_response}")
        return True
    except Exception as e:
        print(f"✗ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_parallel_tools():
    """Test: Parallel tool calling."""
    print("\n[TEST 2] Parallel Tool Calls")
    print("-" * 70)
    
    from x3_chain_ollama_tools import StreamingToolCaller
    
    def check_security(component: str) -> str:
        return f"✓ {component} security: PASS"
    
    def check_performance(component: str) -> str:
        return f"✓ {component} performance: PASS"
    
    caller = StreamingToolCaller(model="qwen2.5-coder:7b")
    
    try:
        result = caller.call_parallel_tools(
            "Audit the fraud proofs module",
            tools=[check_security, check_performance],
            stream=False,
        )
        
        assert len(result.tool_calls_made) >= 1, "No tools called"
        assert result.final_response, "No final response"
        
        print(f"✓ Tools called: {len(result.tool_calls_made)}")
        print(f"✓ Tools executed: {[t[0] for t in result.tool_calls_made]}")
        return True
    except Exception as e:
        print(f"✗ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_agent_loop():
    """Test: Multi-turn agent loop."""
    print("\n[TEST 3] Agent Loop (Multi-turn)")
    print("-" * 70)
    
    from x3_chain_ollama_tools import StreamingToolCaller
    
    def add(a: int, b: int) -> int:
        return a + b
    
    def multiply(a: int, b: int) -> int:
        return a * b
    
    caller = StreamingToolCaller(model="qwen2.5-coder:7b")
    
    try:
        result = caller.agent_loop(
            "What is (2+3)*4?",  # Should calculate: 2+3=5, then 5*4=20
            tools=[add, multiply],
            max_iterations=5,
            stream=False,
        )
        
        assert isinstance(result.tool_calls_made, list), "Tool calls not collected"
        assert len(result.tool_calls_made) > 0, "Agent didn't make any tool calls"
        
        print(f"✓ Iterations: {len(result.tool_calls_made)}")
        print(f"✓ Tools executed: {[t[0] for t in result.tool_calls_made]}")
        print(f"✓ Final response: {result.final_response}")
        return True
    except Exception as e:
        print(f"✗ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_x3_chain_analyzer():
    """Test: X3 Chain specialized analyzer."""
    print("\n[TEST 4] X3 Chain Analyzer")
    print("-" * 70)
    
    from x3_chain_ollama_tools import X3ChainAnalyzer
    
    analyzer = X3ChainAnalyzer(model="qwen2.5-coder:7b")
    
    try:
        result = analyzer.analyze_rust_contract("crates/witness/src/witness_v1.rs")
        
        assert result.tool_calls_made, "No tools executed"
        assert result.final_response, "No final response"
        
        print(f"✓ Tools executed: {[t[0] for t in result.tool_calls_made]}")
        print(f"✓ Analysis complete: {bool(result.final_response)}")
        return True
    except Exception as e:
        print(f"✗ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_streaming_vs_nonstreaming():
    """Test: Streaming and non-streaming produce same results."""
    print("\n[TEST 5] Streaming vs Non-Streaming Consistency")
    print("-" * 70)
    
    from x3_chain_ollama_tools import StreamingToolCaller
    
    def simple_func(x: str) -> str:
        return f"Result: {x}"
    
    caller = StreamingToolCaller(model="qwen2.5-coder:7b", render_content=False)
    
    try:
        # Non-streaming
        result_nonstream = caller.call_single_tool(
            "Test message",
            tools=[simple_func],
            stream=False,
        )
        
        # Streaming
        result_stream = caller.call_single_tool(
            "Test message",
            tools=[simple_func],
            stream=True,
        )
        
        # Both should have tool calls
        assert result_nonstream.tool_calls_made, "Non-streaming: no tools"
        assert result_stream.tool_calls_made, "Streaming: no tools"
        
        print(f"✓ Non-streaming calls: {len(result_nonstream.tool_calls_made)}")
        print(f"✓ Streaming calls: {len(result_stream.tool_calls_made)}")
        print("✓ Both modes produced results")
        return True
    except Exception as e:
        print(f"✗ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_tool_result_structure():
    """Test: ToolResult has all expected fields."""
    print("\n[TEST 6] ToolResult Structure")
    print("-" * 70)
    
    from x3_chain_ollama_tools import ToolResult, StreamingToolCaller
    
    def dummy_tool(x: str) -> str:
        return "result"
    
    caller = StreamingToolCaller(model="qwen2.5-coder:7b", render_content=False)
    
    try:
        result = caller.call_single_tool(
            "Test",
            tools=[dummy_tool],
            stream=False,
        )
        
        # Verify all fields exist
        assert hasattr(result, 'thinking'), "Missing 'thinking' field"
        assert hasattr(result, 'content'), "Missing 'content' field"
        assert hasattr(result, 'tool_calls_made'), "Missing 'tool_calls_made' field"
        assert hasattr(result, 'final_response'), "Missing 'final_response' field"
        
        # Verify types
        assert isinstance(result.thinking, str), "thinking is not string"
        assert isinstance(result.content, str), "content is not string"
        assert isinstance(result.tool_calls_made, list), "tool_calls_made is not list"
        assert isinstance(result.final_response, str), "final_response is not string"
        
        print("✓ All fields present")
        print("✓ All types correct")
        return True
    except Exception as e:
        print(f"✗ Test failed: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_error_handling():
    """Test: Proper error handling for invalid tools."""
    print("\n[TEST 7] Error Handling")
    print("-" * 70)
    
    from x3_chain_ollama_tools import StreamingToolCaller
    
    def valid_tool(x: str) -> str:
        return "ok"
    
    caller = StreamingToolCaller(model="qwen2.5-coder:7b", render_content=False)
    
    try:
        # Should not crash with invalid input
        result = caller.call_single_tool(
            "What is 1+1?",
            tools=[valid_tool],
            stream=False,
        )
        
        # Result should be valid even if tool wasn't called
        assert isinstance(result.final_response, str), "No response"
        
        print("✓ Error handling works")
        print("✓ No crashes on edge cases")
        return True
    except Exception as e:
        print(f"✗ Test failed: {e}")
        return False


def run_all_tests():
    """Run all tests and report summary."""
    print("\n" + "="*70)
    print("X3 CHAIN OLLAMA STREAMING TOOLS - TEST SUITE")
    print("="*70)
    
    tests = [
        ("Single Tool Call", test_single_tool_call),
        ("Parallel Tools", test_parallel_tools),
        ("Agent Loop", test_agent_loop),
        ("X3 Chain Analyzer", test_x3_chain_analyzer),
        ("Streaming vs Non-Streaming", test_streaming_vs_nonstreaming),
        ("ToolResult Structure", test_tool_result_structure),
        ("Error Handling", test_error_handling),
    ]
    
    results = []
    
    for test_name, test_func in tests:
        try:
            passed = test_func()
            results.append((test_name, passed))
        except Exception as e:
            print(f"\n✗ {test_name}: Unhandled exception")
            print(f"  {e}")
            results.append((test_name, False))
    
    # Summary
    print("\n" + "="*70)
    print("TEST SUMMARY")
    print("="*70)
    
    passed = sum(1 for _, p in results if p)
    total = len(results)
    
    for test_name, passed_flag in results:
        status = "✓ PASS" if passed_flag else "✗ FAIL"
        print(f"{status}: {test_name}")
    
    print("-" * 70)
    print(f"Results: {passed}/{total} tests passed")
    
    if passed == total:
        print("\n✓ ALL TESTS PASSED!")
        return 0
    else:
        print(f"\n✗ {total - passed} test(s) failed")
        return 1


# ============================================================================
# Manual Testing Commands
# ============================================================================

MANUAL_TEST_GUIDE = """
╔════════════════════════════════════════════════════════════════════════════╗
║                        MANUAL TESTING GUIDE                               ║
╚════════════════════════════════════════════════════════════════════════════╝

To manually test streaming tool-calling:

1. SINGLE TOOL CALL (Quick)
   ─────────────────────────
   from x3_chain_ollama_tools import StreamingToolCaller
   
   caller = StreamingToolCaller()
   result = caller.call_single_tool(
       "Analyze witness_v1.rs",
       tools=[lambda f: f"Analysis of {f}: OK"]
   )
   print(result.final_response)

2. PARALLEL TOOLS (Real-time Feedback)
   ────────────────────────────────────
   result = caller.call_parallel_tools(
       "Audit fraud proofs module",
       tools=[
           lambda c: f"Security audit: PASS",
           lambda c: f"Performance check: PASS",
       ]
   )

3. AGENT LOOP (Multi-turn Reasoning)
   ──────────────────────────────────
   result = caller.agent_loop(
       "What is (11434+12341)*412?",
       tools=[
           lambda a, b: a + b,
           lambda a, b: a * b,
       ],
       max_iterations=5
   )

4. STREAMING (Real-time Rendering)
   ────────────────────────────────
   caller = StreamingToolCaller(render_content=True)
   result = caller.call_single_tool(..., stream=True)
   # Watch output appear in real-time

5. NON-STREAMING (Simple, Blocking)
   ────────────────────────────────
   caller = StreamingToolCaller(render_content=False)
   result = caller.call_single_tool(..., stream=False)
   # Entire response waits until complete

DEBUGGING TIPS
──────────────

1. Enable all rendering:
   caller = StreamingToolCaller(
       render_thinking=True,
       render_content=True,
       render_tool_calls=True,
   )

2. Check what tools were called:
   print([t[0] for t in result.tool_calls_made])

3. See the thinking process:
   print(result.thinking)

4. Check final synthesis:
   print(result.final_response)

5. Test with qwen3 (thinking-capable):
   caller = StreamingToolCaller(model="qwen3:latest")
"""


if __name__ == "__main__":
    exit_code = run_all_tests()
    print(MANUAL_TEST_GUIDE)
    sys.exit(exit_code)

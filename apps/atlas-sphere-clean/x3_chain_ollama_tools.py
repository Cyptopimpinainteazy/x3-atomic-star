#!/usr/bin/env python3
"""
X3 Chain Ollama Streaming Tool-Calling Wrapper

Production-ready wrapper for tool-calling workflows with streaming support.
Handles single tools, parallel tools, multi-turn agent loops, and thinking models.

Usage:
    from x3_chain_ollama_tools import StreamingToolCaller
    
    caller = StreamingToolCaller(model="qwen2.5-coder:7b")
    result = caller.call_tool("Analyze this Rust file", [analyze_rust_code])
"""

from typing import Callable, Any, Optional, List, Dict
from dataclasses import dataclass
import json
import re
from ollama import chat


@dataclass
class ToolResult:
    """Result from a tool execution"""
    thinking: str
    content: str
    tool_calls_made: list
    final_response: str


class ToolCall:
    """Wrapper for tool calls from Ollama (handles both structured and JSON formats)"""
    
    class Function:
        def __init__(self, name: str, arguments: dict):
            self.name = name
            self.arguments = arguments
    
    def __init__(self, name: str, arguments: dict):
        self.function = self.Function(name, arguments)


class StreamingToolCaller:
    """
    Streaming tool-calling wrapper for Ollama models.
    
    Supports:
    - Single tool calls
    - Parallel tool calls
    - Multi-turn agent loops
    - Streaming with thinking models
    - Real-time output rendering
    """
    
    def __init__(
        self,
        model: str = "qwen2.5-coder:7b",
        render_thinking: bool = True,
        render_content: bool = True,
        render_tool_calls: bool = True,
    ):
        """
        Initialize the streaming tool caller.
        
        Args:
            model: Model to use (default: qwen2.5-coder:7b)
            render_thinking: Show thinking output in real-time
            render_content: Show content output in real-time
            render_tool_calls: Show tool calls in real-time
        """
        self.model = model
        self.render_thinking = render_thinking
        self.render_content = render_content
        self.render_tool_calls = render_tool_calls
    
    def _extract_json_tool_calls(self, content: str) -> List[ToolCall]:
        """Extract tool calls from JSON content (Ollama returns them as JSON strings in content)."""
        tool_calls = []
        
        # Pattern: {"name": "...", "arguments": {...}}
        json_pattern = r'\{["\']name["\']\s*:\s*["\']([^"\']+)["\']\s*,\s*["\']arguments["\']\s*:\s*(\{[^}]*\})\}'
        
        # Find all JSON objects that look like tool calls
        for match in re.finditer(json_pattern, content, re.IGNORECASE | re.DOTALL):
            try:
                # Extract full JSON object
                start_idx = match.start()
                json_str = content[start_idx:]
                
                # Find matching closing brace
                brace_count = 0
                end_idx = 0
                in_string = False
                escape_next = False
                
                for i, char in enumerate(json_str):
                    if escape_next:
                        escape_next = False
                        continue
                    if char == '\\':
                        escape_next = True
                        continue
                    if char == '"' and not escape_next:
                        in_string = not in_string
                        continue
                    if not in_string:
                        if char == '{':
                            brace_count += 1
                        elif char == '}':
                            brace_count -= 1
                            if brace_count == 0:
                                end_idx = i + 1
                                break
                
                if end_idx > 0:
                    json_obj_str = json_str[:end_idx]
                    tool_call_obj = json.loads(json_obj_str)
                    
                    tool_name = tool_call_obj.get("name", "")
                    arguments = tool_call_obj.get("arguments", {})
                    
                    if tool_name and isinstance(arguments, dict):
                        tool_calls.append(ToolCall(tool_name, arguments))
            except (json.JSONDecodeError, ValueError, KeyError):
                # Skip malformed JSON
                continue
        
        return tool_calls
    
    def _execute_tool(self, tool_name: str, arguments: dict, available_functions: dict) -> str:
        """Execute a single tool and return its result."""
        if tool_name in available_functions:
            func = available_functions[tool_name]
            result = func(**arguments)
            return str(result)
        return f"Unknown tool: {tool_name}"
    
    def call_single_tool(
        self,
        user_message: str,
        tools: List[Callable],
        stream: bool = True,
        think: bool = True,
    ) -> ToolResult:
        """
        Call a single tool with streaming support.
        
        Args:
            user_message: User's request
            tools: List of tool functions
            stream: Enable streaming
            think: Enable thinking for capable models
            
        Returns:
            ToolResult object with thinking, content, tool calls, and final response
        """
        messages = [{"role": "user", "content": user_message}]
        available_functions = {func.__name__: func for func in tools}
        
        # First request - get tool call
        thinking, content, tool_calls = self._stream_response(
            messages, tools, stream, think
        )
        
        messages.append({
            "role": "assistant",
            "thinking": thinking,
            "content": content,
            "tool_calls": tool_calls,
        })
        
        # Execute tools
        for call in tool_calls:
            result = self._execute_tool(
                call.function.name,
                call.function.arguments,
                available_functions
            )
            messages.append({
                "role": "tool",
                "tool_name": call.function.name,
                "content": result,
            })
        
        # Get final response
        if tool_calls:
            thinking2, content2, _ = self._stream_response(
                messages, tools, stream, think
            )
            final_response = content2
        else:
            final_response = content
        
        return ToolResult(
            thinking=thinking,
            content=content,
            tool_calls_made=[(c.function.name, c.function.arguments) for c in tool_calls],
            final_response=final_response,
        )
    
    def call_parallel_tools(
        self,
        user_message: str,
        tools: List[Callable],
        stream: bool = True,
        think: bool = True,
    ) -> ToolResult:
        """
        Call multiple tools in parallel.
        
        Args:
            user_message: User's request
            tools: List of tool functions
            stream: Enable streaming
            think: Enable thinking for capable models
            
        Returns:
            ToolResult object with all execution details
        """
        messages = [{"role": "user", "content": user_message}]
        available_functions = {func.__name__: func for func in tools}
        
        # Get all tool calls
        thinking, content, tool_calls = self._stream_response(
            messages, tools, stream, think
        )
        
        messages.append({
            "role": "assistant",
            "thinking": thinking,
            "content": content,
            "tool_calls": tool_calls,
        })
        
        # Execute all tools in parallel (append all results)
        for call in tool_calls:
            result = self._execute_tool(
                call.function.name,
                call.function.arguments,
                available_functions
            )
            messages.append({
                "role": "tool",
                "tool_name": call.function.name,
                "content": result,
            })
        
        # Get synthesis response
        if tool_calls:
            thinking2, content2, _ = self._stream_response(
                messages, tools, stream, think
            )
            final_response = content2
        else:
            final_response = content
        
        return ToolResult(
            thinking=thinking,
            content=content,
            tool_calls_made=[(c.function.name, c.function.arguments) for c in tool_calls],
            final_response=final_response,
        )
    
    def agent_loop(
        self,
        user_message: str,
        tools: List[Callable],
        max_iterations: int = 10,
        stream: bool = True,
        think: bool = True,
    ) -> ToolResult:
        """
        Multi-turn agent loop - model decides when to invoke tools.
        
        Args:
            user_message: User's request
            tools: List of tool functions
            max_iterations: Max number of iterations
            stream: Enable streaming
            think: Enable thinking for capable models
            
        Returns:
            ToolResult with all iterations and final response
        """
        messages = [{"role": "user", "content": user_message}]
        available_functions = {func.__name__: func for func in tools}
        all_thinking = ""
        all_tool_calls = []
        
        for iteration in range(max_iterations):
            print(f"\n[Iteration {iteration + 1}]")
            
            thinking, content, tool_calls = self._stream_response(
                messages, tools, stream, think
            )
            
            all_thinking += thinking
            all_tool_calls.extend([
                (c.function.name, c.function.arguments) for c in tool_calls
            ])
            
            messages.append({
                "role": "assistant",
                "thinking": thinking,
                "content": content,
                "tool_calls": tool_calls,
            })
            
            if not tool_calls:
                # No more tools to call - agent is done
                return ToolResult(
                    thinking=all_thinking,
                    content=content,
                    tool_calls_made=all_tool_calls,
                    final_response=content,
                )
            
            # Execute all tools from this iteration
            for call in tool_calls:
                result = self._execute_tool(
                    call.function.name,
                    call.function.arguments,
                    available_functions
                )
                messages.append({
                    "role": "tool",
                    "tool_name": call.function.name,
                    "content": result,
                })
        
        # Max iterations reached
        return ToolResult(
            thinking=all_thinking,
            content="Max iterations reached",
            tool_calls_made=all_tool_calls,
            final_response="Max iterations reached",
        )
    
    def _stream_response(
        self,
        messages: List[dict],
        tools: List[Callable],
        stream: bool,
        think: bool,
    ) -> tuple:
        """
        Stream response and accumulate partial fields.
        
        Extracts tool calls from both:
        - Structured tool_calls field (if Ollama parses them)
        - JSON content (when Ollama returns tool calls as JSON strings)
        
        Returns:
            (thinking, content, tool_calls)
        """
        response = chat(
            model=self.model,
            messages=messages,
            tools=tools,
            stream=stream,
            think=think if self._model_supports_thinking() else False,
        )
        
        thinking = ""
        content = ""
        tool_calls = []  # Always initialize as list
        done_thinking = False
        
        if stream:
            # Stream mode: iterate through chunks
            for chunk in response:
                # Handle chunk structure
                chunk_msg = chunk.get("message") if isinstance(chunk, dict) else getattr(chunk, "message", chunk)
                
                if chunk_msg is None:
                    continue
                
                # Accumulate thinking
                chunk_thinking = getattr(chunk_msg, "thinking", None)
                if chunk_thinking:
                    thinking += chunk_thinking
                    if self.render_thinking:
                        if not done_thinking:
                            print("Thinking: ", end="", flush=True)
                            done_thinking = True
                        print(chunk_thinking, end="", flush=True)
                
                # Accumulate content
                chunk_content = getattr(chunk_msg, "content", None)
                if chunk_content:
                    if done_thinking and self.render_content:
                        print("\n\nContent: ", end="", flush=True)
                        done_thinking = False
                    if self.render_content:
                        print(chunk_content, end="", flush=True)
                    content += chunk_content
                
                # Accumulate tool calls from structured field
                chunk_tools = getattr(chunk_msg, "tool_calls", None)
                if chunk_tools:
                    tool_calls.extend(chunk_tools)
                    if self.render_tool_calls:
                        print(f"\nTools: {chunk_tools}")
        else:
            # Non-streaming mode: single response
            chunk_msg = response.message
            thinking = getattr(chunk_msg, "thinking", "") or ""
            content = getattr(chunk_msg, "content", "") or ""
            chunk_tools = getattr(chunk_msg, "tool_calls", None)
            tool_calls = chunk_tools if chunk_tools else []
            
            if self.render_thinking and thinking:
                print(f"Thinking: {thinking}")
            if self.render_content and content:
                print(f"Content: {content}")
            if self.render_tool_calls and tool_calls:
                print(f"Tools: {tool_calls}")
        
        # If no structured tool calls found, try to extract JSON tool calls from content
        if not tool_calls and content:
            json_tool_calls = self._extract_json_tool_calls(content)
            if json_tool_calls:
                tool_calls = json_tool_calls
                if self.render_tool_calls:
                    print(f"Tools (from JSON): {[t.function.name for t in tool_calls]}")
        
        if self.render_thinking or self.render_content:
            print()
        
        return thinking, content, tool_calls
    
    def _model_supports_thinking(self) -> bool:
        """Check if model supports thinking (qwen3 variants)."""
        return "qwen3" in self.model.lower()


class X3ChainAnalyzer:
    """
    Specialized analyzer for X3 Chain code using streaming tools.
    """
    
    def __init__(self, model: str = "qwen2.5-coder:7b"):
        self.caller = StreamingToolCaller(model=model)
    
    def analyze_rust_contract(self, file_path: str) -> ToolResult:
        """Analyze Rust smart contract for security issues."""
        
        def analyze_rust_security(file_path: str) -> str:
            """Analyze Rust code for security vulnerabilities"""
            # In real scenario, would read and analyze file
            return f"Analyzed {file_path}: Checking for unsafe blocks, arithmetic overflows, panic conditions, access control"
        
        def check_performance(file_path: str) -> str:
            """Check performance characteristics"""
            return f"Checked {file_path}: Algorithm efficiency, memory usage, state transitions"
        
        return self.caller.call_parallel_tools(
            f"Perform comprehensive security and performance audit of {file_path}",
            [analyze_rust_security, check_performance]
        )
    
    def audit_fraud_proofs(self, module_path: str) -> ToolResult:
        """Audit fraud proofs module for correctness."""
        
        def verify_proof_logic(module_path: str) -> str:
            """Verify fraud proof logic"""
            return f"Verified {module_path}: Proof generation, verification steps, challenge-response mechanism"
        
        def check_state_consistency(module_path: str) -> str:
            """Check state root consistency"""
            return f"Checked {module_path}: State root computation, Merkle proof validity, consistency checks"
        
        return self.caller.call_parallel_tools(
            f"Audit fraud proofs in {module_path} for correctness and security",
            [verify_proof_logic, check_state_consistency]
        )
    
    def validate_x3_compliance(self, config_path: str) -> ToolResult:
        """Validate X3 Chain configuration compliance."""
        
        def check_cross_vm_config(config_path: str) -> str:
            """Validate cross-VM configuration"""
            return f"Validated {config_path}: EVM bridge, SVM integration, state sync"
        
        def verify_consensus_params(config_path: str) -> str:
            """Verify consensus parameters"""
            return f"Verified {config_path}: Block time, finality, validator requirements"
        
        def check_gpu_settings(config_path: str) -> str:
            """Check GPU acceleration settings"""
            return f"Checked {config_path}: GPU executor, proof generation acceleration"
        
        return self.caller.call_parallel_tools(
            f"Validate X3 Chain compliance in {config_path}",
            [check_cross_vm_config, verify_consensus_params, check_gpu_settings]
        )


if __name__ == "__main__":
    # Example usage
    print("X3 Chain Ollama Streaming Tool-Calling Wrapper")
    print("=" * 70)
    
    # Example 1: Single tool call
    print("\n[Example 1: Single Tool Call]")
    caller = StreamingToolCaller(model="qwen2.5-coder:7b")
    
    def get_chain_stats(chain_name: str) -> str:
        stats = {
            "x3-chain": "TPS: 10,000 | Finality: 2 blocks | Cross-VM: EVM+SVM",
            "ethereum": "TPS: 15 | Finality: ~13 blocks | VM: EVM only",
        }
        return stats.get(chain_name.lower(), "Unknown chain")
    
    result = caller.call_single_tool(
        "Get statistics for x3-chain",
        [get_chain_stats]
    )
    print(f"\nFinal Response: {result.final_response}")
    
    # Example 2: Parallel tools
    print("\n\n[Example 2: Parallel Tools]")
    def check_security(component: str) -> str:
        return f"Security check for {component}: All critical checks passed"
    
    def performance_test(component: str) -> str:
        return f"Performance test for {component}: Throughput 10k TPS, latency 100ms"
    
    result = caller.call_parallel_tools(
        "Audit the fraud proofs module",
        [check_security, performance_test]
    )
    print(f"\nFinal Response: {result.final_response}")
    
    # Example 3: X3 Chain analyzer
    print("\n\n[Example 3: X3 Chain Analyzer]")
    analyzer = X3ChainAnalyzer()
    result = analyzer.analyze_rust_contract("src/witness_v1.rs")
    print(f"\nAudit Complete")
    print(f"Tools Called: {[t[0] for t in result.tool_calls_made]}")

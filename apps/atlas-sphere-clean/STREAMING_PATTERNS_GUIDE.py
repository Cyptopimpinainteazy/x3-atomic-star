#!/usr/bin/env python3
"""
Ollama Tool-Calling Patterns: Streaming vs Non-Streaming

This module demonstrates the key differences and tradeoffs between
streaming and non-streaming tool-calling for X3 Chain workflows.
"""

from ollama import chat


# ============================================================================
# PATTERN 1: NON-STREAMING TOOL CALLING (Simple, Blocking)
# ============================================================================

def non_streaming_single_tool():
    """Non-streaming single tool call - simple but blocks until complete."""
    
    def get_temperature(city: str) -> str:
        temps = {"New York": "22°C", "London": "15°C", "Tokyo": "18°C"}
        return temps.get(city, "Unknown")
    
    messages = [{"role": "user", "content": "What is the temperature in New York?"}]
    
    # First request - blocking, waits for entire response
    response = chat(
        model="qwen2.5-coder:7b",
        messages=messages,
        tools=[get_temperature],
        stream=False,  # BLOCKING - waits for full response
    )
    
    messages.append(response.message)
    
    # Check if tools were called
    if response.message.tool_calls:
        call = response.message.tool_calls[0]
        result = get_temperature(**call.function.arguments)
        messages.append({"role": "tool", "tool_name": call.function.name, "content": result})
        
        # Second request - blocking again
        final_response = chat(
            model="qwen2.5-coder:7b",
            messages=messages,
            tools=[get_temperature],
            stream=False,
        )
        return final_response.message.content
    
    return response.message.content


# ============================================================================
# PATTERN 2: STREAMING TOOL CALLING (Real-time, Responsive)
# ============================================================================

def streaming_single_tool():
    """Streaming single tool call - renders output as it arrives."""
    
    def get_temperature(city: str) -> str:
        temps = {"New York": "22°C", "London": "15°C", "Tokyo": "18°C"}
        return temps.get(city, "Unknown")
    
    messages = [{"role": "user", "content": "What is the temperature in New York?"}]
    
    # First request - streaming, renders in real-time
    stream = chat(
        model="qwen2.5-coder:7b",
        messages=messages,
        tools=[get_temperature],
        stream=True,  # STREAMING - yields chunks as available
    )
    
    thinking = ""
    content = ""
    tool_calls = []
    
    # Real-time rendering as chunks arrive
    for chunk in stream:
        if chunk.message.thinking:
            thinking += chunk.message.thinking
            print(chunk.message.thinking, end="", flush=True)
        
        if chunk.message.content:
            content += chunk.message.content
            print(chunk.message.content, end="", flush=True)
        
        if chunk.message.tool_calls:
            tool_calls.extend(chunk.message.tool_calls)
    
    print()  # newline
    
    # Append accumulated fields
    messages.append({
        "role": "assistant",
        "thinking": thinking,
        "content": content,
        "tool_calls": tool_calls,
    })
    
    # Execute tools
    for call in tool_calls:
        result = get_temperature(**call.function.arguments)
        messages.append({
            "role": "tool",
            "tool_name": call.function.name,
            "content": result,
        })
    
    # Second request - also streaming
    stream2 = chat(
        model="qwen2.5-coder:7b",
        messages=messages,
        tools=[get_temperature],
        stream=True,
    )
    
    final_content = ""
    for chunk in stream2:
        if chunk.message.content:
            final_content += chunk.message.content
            print(chunk.message.content, end="", flush=True)
    
    print()
    return final_content


# ============================================================================
# PATTERN 3: NON-STREAMING PARALLEL TOOLS (Blocking, Simple)
# ============================================================================

def non_streaming_parallel_tools():
    """Non-streaming parallel tools - executes all in one request."""
    
    def get_temperature(city: str) -> str:
        temps = {"New York": "22°C", "London": "15°C", "Tokyo": "18°C"}
        return temps.get(city, "Unknown")
    
    def get_conditions(city: str) -> str:
        conds = {"New York": "Cloudy", "London": "Rainy", "Tokyo": "Sunny"}
        return conds.get(city, "Unknown")
    
    messages = [{
        "role": "user",
        "content": "What's the weather in New York and London?"
    }]
    
    # Single blocking request returns multiple tool calls
    response = chat(
        model="qwen2.5-coder:7b",
        messages=messages,
        tools=[get_temperature, get_conditions],
        stream=False,  # BLOCKING
    )
    
    messages.append(response.message)
    
    # Execute all tools at once
    for call in response.message.tool_calls:
        if call.function.name == "get_temperature":
            result = get_temperature(**call.function.arguments)
        elif call.function.name == "get_conditions":
            result = get_conditions(**call.function.arguments)
        else:
            result = "Unknown tool"
        
        messages.append({
            "role": "tool",
            "tool_name": call.function.name,
            "content": result,
        })
    
    # Final synthesis
    final_response = chat(
        model="qwen2.5-coder:7b",
        messages=messages,
        tools=[get_temperature, get_conditions],
        stream=False,
    )
    
    return final_response.message.content


# ============================================================================
# PATTERN 4: STREAMING PARALLEL TOOLS (Real-time, All Tools)
# ============================================================================

def streaming_parallel_tools():
    """Streaming parallel tools - renders as model generates tool calls."""
    
    def get_temperature(city: str) -> str:
        temps = {"New York": "22°C", "London": "15°C", "Tokyo": "18°C"}
        return temps.get(city, "Unknown")
    
    def get_conditions(city: str) -> str:
        conds = {"New York": "Cloudy", "London": "Rainy", "Tokyo": "Sunny"}
        return conds.get(city, "Unknown")
    
    messages = [{
        "role": "user",
        "content": "What's the weather in New York and London?"
    }]
    
    # Streaming request - renders tool calls as they're generated
    stream = chat(
        model="qwen2.5-coder:7b",
        messages=messages,
        tools=[get_temperature, get_conditions],
        stream=True,  # STREAMING
    )
    
    thinking = ""
    content = ""
    tool_calls = []
    
    for chunk in stream:
        if chunk.message.thinking:
            thinking += chunk.message.thinking
            print(f"[Thinking] {chunk.message.thinking}", flush=True)
        
        if chunk.message.content:
            content += chunk.message.content
            print(chunk.message.content, end="", flush=True)
        
        # Tool calls appear as they're generated
        if chunk.message.tool_calls:
            tool_calls.extend(chunk.message.tool_calls)
            for call in chunk.message.tool_calls:
                print(f"\n[Tool Call] {call.function.name}({call.function.arguments})")
    
    print()
    
    messages.append({
        "role": "assistant",
        "thinking": thinking,
        "content": content,
        "tool_calls": tool_calls,
    })
    
    # Execute all tools in parallel
    for call in tool_calls:
        if call.function.name == "get_temperature":
            result = get_temperature(**call.function.arguments)
        elif call.function.name == "get_conditions":
            result = get_conditions(**call.function.arguments)
        else:
            result = "Unknown tool"
        
        messages.append({
            "role": "tool",
            "tool_name": call.function.name,
            "content": result,
        })
    
    # Stream final synthesis
    stream2 = chat(
        model="qwen2.5-coder:7b",
        messages=messages,
        tools=[get_temperature, get_conditions],
        stream=True,
    )
    
    final_content = ""
    for chunk in stream2:
        if chunk.message.content:
            final_content += chunk.message.content
            print(chunk.message.content, end="", flush=True)
    
    print()
    return final_content


# ============================================================================
# PATTERN 5: NON-STREAMING AGENT LOOP (Simple Multi-turn)
# ============================================================================

def non_streaming_agent_loop():
    """Non-streaming agent loop - simpler but model must wait between iterations."""
    
    def add(a: int, b: int) -> int:
        return a + b
    
    def multiply(a: int, b: int) -> int:
        return a * b
    
    messages = [{"role": "user", "content": "What is (5+3)*2?"}]
    
    while True:
        response = chat(
            model="qwen2.5-coder:7b",
            messages=messages,
            tools=[add, multiply],
            stream=False,  # BLOCKING each iteration
        )
        
        messages.append(response.message)
        
        if not response.message.tool_calls:
            return response.message.content
        
        for call in response.message.tool_calls:
            if call.function.name == "add":
                result = add(**call.function.arguments)
            elif call.function.name == "multiply":
                result = multiply(**call.function.arguments)
            else:
                result = "Unknown tool"
            
            messages.append({
                "role": "tool",
                "tool_name": call.function.name,
                "content": str(result),
            })


# ============================================================================
# PATTERN 6: STREAMING AGENT LOOP (Real-time Multi-turn)
# ============================================================================

def streaming_agent_loop():
    """Streaming agent loop - responsive across multiple iterations."""
    
    def add(a: int, b: int) -> int:
        return a + b
    
    def multiply(a: int, b: int) -> int:
        return a * b
    
    messages = [{"role": "user", "content": "What is (5+3)*2?"}]
    iteration = 0
    
    while True:
        iteration += 1
        print(f"\n[Iteration {iteration}]")
        
        stream = chat(
            model="qwen2.5-coder:7b",
            messages=messages,
            tools=[add, multiply],
            stream=True,  # STREAMING each iteration
        )
        
        thinking = ""
        content = ""
        tool_calls = []
        
        for chunk in stream:
            if chunk.message.thinking:
                thinking += chunk.message.thinking
            
            if chunk.message.content:
                content += chunk.message.content
                print(chunk.message.content, end="", flush=True)
            
            if chunk.message.tool_calls:
                tool_calls.extend(chunk.message.tool_calls)
                for call in chunk.message.tool_calls:
                    print(f" [→ {call.function.name}]", end="", flush=True)
        
        print()
        
        messages.append({
            "role": "assistant",
            "thinking": thinking,
            "content": content,
            "tool_calls": tool_calls,
        })
        
        if not tool_calls:
            return content
        
        for call in tool_calls:
            if call.function.name == "add":
                result = add(**call.function.arguments)
            elif call.function.name == "multiply":
                result = multiply(**call.function.arguments)
            else:
                result = "Unknown tool"
            
            print(f"  {call.function.name}({call.function.arguments}) → {result}")
            messages.append({
                "role": "tool",
                "tool_name": call.function.name,
                "content": str(result),
            })


# ============================================================================
# COMPARISON SUMMARY
# ============================================================================

COMPARISON = """
╔═══════════════════════════════════════════════════════════════════════════╗
║                 STREAMING vs NON-STREAMING COMPARISON                     ║
╠═══════════════════════════════════════════════════════════════════════════╣

┌─────────────────────────────────────────────────────────────────────────┐
│ NON-STREAMING (stream=False)                                            │
├─────────────────────────────────────────────────────────────────────────┤
│ ✓ Simple implementation - full response at once                         │
│ ✓ Easy to debug - all data received before processing                  │
│ ✓ Good for short responses                                             │
│ ✓ Easier error handling                                                │
│                                                                         │
│ ✗ Blocking - CLI hangs waiting for model                               │
│ ✗ High perceived latency - user sees nothing until done                │
│ ✗ Worst for long generations (poems, code, etc)                        │
│ ✗ Bad UX - no feedback to user while processing                        │
│                                                                         │
│ Best for: Batch processing, non-interactive tools, simple requests     │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│ STREAMING (stream=True)                                                 │
├─────────────────────────────────────────────────────────────────────────┤
│ ✓ Real-time feedback - user sees output immediately                    │
│ ✓ Lower perceived latency - feels snappy                               │
│ ✓ Better for long generations                                          │
│ ✓ Can cancel mid-generation                                            │
│ ✓ Shows tool calls as they appear                                      │
│ ✓ Shows thinking progress for capable models                           │
│                                                                         │
│ ✗ More complex accumulation logic                                      │
│ ✗ Must handle partial chunks correctly                                 │
│ ✗ Requires careful message reconstruction                              │
│ ✗ Token counting more complex                                          │
│                                                                         │
│ Best for: Interactive tools, CLI, long responses, agent loops          │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│ KEY ACCUMULATION REQUIREMENT (Critical for Streaming)                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│ When streaming with tools or thinking, ACCUMULATE all partial fields:  │
│                                                                         │
│   thinking = ""                                                        │
│   content = ""                                                         │
│   tool_calls = []                                                      │
│                                                                         │
│   for chunk in stream:                                                 │
│       if chunk.message.thinking:                                       │
│           thinking += chunk.message.thinking    ← ACCUMULATE          │
│       if chunk.message.content:                                        │
│           content += chunk.message.content      ← ACCUMULATE          │
│       if chunk.message.tool_calls:                                     │
│           tool_calls.extend(chunk...)           ← ACCUMULATE          │
│                                                                         │
│   # Then pass ALL accumulated fields back in next request:             │
│   messages.append({                                                    │
│       'role': 'assistant',                                             │
│       'thinking': thinking,        ← COMPLETE THINKING                │
│       'content': content,          ← COMPLETE CONTENT                 │
│       'tool_calls': tool_calls,    ← ALL TOOL CALLS                   │
│   })                                                                    │
│                                                                         │
│ This is REQUIRED for tool calling to work properly!                    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘

╔═══════════════════════════════════════════════════════════════════════════╗
║                          X3 CHAIN RECOMMENDATIONS                         ║
╠═══════════════════════════════════════════════════════════════════════════╣

For X3 Chain workflows:

1. CODE AUDITS & SECURITY ANALYSIS
   → Use STREAMING with parallel tools
   → Reason: Model provides running commentary, shows tools as called
   → Pattern: streaming_parallel_tools()

2. INTERACTIVE CLI TOOLS
   → Use STREAMING with single/parallel tools
   → Reason: Users need immediate feedback
   → Pattern: streaming_single_tool() or streaming_parallel_tools()

3. BATCH PROCESSING (Non-interactive)
   → Use NON-STREAMING for simplicity
   → Reason: No user waiting, easier error handling
   → Pattern: non_streaming_parallel_tools()

4. COMPLEX REASONING (Multi-step)
   → Use STREAMING with agent loop
   → Reason: See each reasoning step, intermediate results
   → Pattern: streaming_agent_loop()

5. FRAUD PROOF VERIFICATION
   → Use STREAMING parallel tools with thinking
   → Reason: Models show reasoning, execute proofs in parallel
   → Pattern: streaming_parallel_tools() with think=True

╚═══════════════════════════════════════════════════════════════════════════╝
"""

print(COMPARISON)

# Setup Claude Code with Ollama

Configure this project to use a local Ollama MCP server.

## What this does

1. Verifies Ollama is installed and running.
2. Verifies at least one local model is available.
3. Adds/updates Claude Code MCP server named `ollama`.
4. Confirms MCP server health.

## Execute

Run the following in the project root:

```bash
command -v ollama >/dev/null 2>&1 || { echo "Ollama is not installed."; exit 1; }
ollama ps >/dev/null 2>&1 || { echo "Ollama daemon is not running."; exit 1; }
ollama list | head -n 20

claude mcp remove ollama >/dev/null 2>&1 || true
claude mcp add ollama -- npx -y ollama-mcp-server

claude mcp list
```

## Notes

- If `ollama list` is empty, pull a model first:

```bash
ollama pull qwen2.5-coder:7b
```

- If you want a specific model behavior, set it in your prompts and task docs (`INITIAL.md`, PRPs).

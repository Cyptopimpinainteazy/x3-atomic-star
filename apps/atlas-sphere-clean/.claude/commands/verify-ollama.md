# Verify Claude Code + Ollama Setup

Run an end-to-end local health check for this repository.

## Execute

```bash
bash scripts/verify_ollama_setup.sh
```

## If verification fails

Run these fallback checks manually:

```bash
ollama --version
ollama ps
ollama list | head -n 20
claude mcp list
```

## Recovery path

If `ollama` MCP is missing or disconnected:

```bash
claude mcp remove ollama >/dev/null 2>&1 || true
claude mcp add ollama -- npx -y ollama-mcp-server
claude mcp list
```

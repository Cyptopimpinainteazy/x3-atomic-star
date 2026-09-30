# X3 AI router

Task feedback: attach `X-X3-Task-ID`, `X-X3-Revision` (full candidate commit SHA), and `X-X3-Scope: router` to chat requests. `/v1/tasks` exposes scoped outcomes, request latency, and attributed spend. IDs bind permanently to agent/revision/scope and cannot be reused after finalization. This version records feedback; it does not automatically change routing based on that feedback.

Set a separate `X3_VERIFIER_TOKEN` on the router and only the verifier worker. On a clean committed checkout, run `python3 services/x3-ai-router/verify_task.py --repo /path/to/xxxstar --task-id TASK`. The verifier runs the fixed router test suite, checks that the checkout stayed unchanged, and submits exit codes/output digests. Builder credentials cannot post outcomes. The trusted verifier credential is a trust boundary; the server cannot independently authenticate the truth of submitted check results. A `checks_passed` result covers only this scope, not a verified merge or blockchain-wide correctness.

The bundled OpenRouter GPT-4.1 Mini and direct GPT-5 prices were checked on 2026-09-29. Paid providers are skipped after 30 days unless `pricing_checked_on` and the rates are refreshed together. The direct provider uses `max_completion_tokens` for GPT-5. Pricing is an estimate; reconcile the dashboard with provider invoices.

Routine requests try local Ollama first, then two explicitly free NVIDIA models on OpenRouter, then paid models. To opt in to the free cloud endpoints, set `X3_ENABLE_FREE_CLOUD=1` and `OPENROUTER_API_KEY`. They have rate/availability limits and must retain a `:free` model ID with zero prices. NVIDIA warns that its free endpoints log prompts for product improvement; never send secrets or confidential code through them. For a local-only setup, remove cloud names from `routes.routine`. Critical requests never use the free models.

Start with `python3 router.py --db /path/to/usage.sqlite3`. Point an OpenAI-compatible client at `http://127.0.0.1:11435/v1`, model `x3-auto`. Set `X3_ROUTER_TOKEN` to require bearer authorization, and set `OPENROUTER_API_KEY` or `OPENAI_API_KEY` for cloud providers. `X-X3-Agent` identifies a caller for per-agent budgets. `GET /v1/usage` returns spend accounting. The service binds to loopback; put authenticated TLS in front of it for remote access.

Edit `config.json` for installed Ollama models, provider models, prices, and budgets. Prices are examples and must be set to current provider rates before relying on cost limits. Paid providers with zero prices are skipped. Requests whose JSON exceeds `max_input_tokens` UTF-8 bytes are rejected as a conservative input bound. SQLite reservations enforce the configured budgets across concurrent workers; a provider returning no usage is charged the reserved estimate. Failed requests are not charged locally even if a provider billed them. Provider-side hidden tokens or prices that change without a config update can still produce a higher actual bill.

## Request validation

A reservation is an upper bound on one call, so the request must not be able to spend more than the reservation covers. Two shapes used to get through:

- **A second output parameter.** The estimate read `max_tokens` and fell back to 4096, so a request that set `max_completion_tokens` instead — which is what GPT-5 on the direct provider requires — was reserved at the default and billed for whatever it asked. Both parameters are now validated against `max_output_tokens`, and the estimate uses whichever one the client set.
- **Multiple completions.** `n` and `best_of` multiply the completions a provider bills for while the estimate assumed one. Anything other than `1` is refused with `400`.

`max_output_tokens` (32768), `default_max_output_tokens` (4096), `reservation_ttl_seconds` (900), `provider_cooldown_seconds` (60) and `provider_cooldown_max_seconds` (3600) are config knobs.

## Provider cooldowns

A provider that fails is skipped for a doubling delay, capped, with `Retry-After` from an HTTP error taking precedence when the provider sends one. Without this, every request in turn paid the timeout of an endpoint that was already down. A success clears the record. `GET /v1/providers` shows consecutive failures and the remaining cooldown, and the skip reason is reported in the `502` body's `attempts`.

## Crash recovery

A reservation is deleted only by `finish`, which runs in the request thread. If the router died between reserving budget and calling the provider, nothing deleted the row: `reserved_usd` grew all day and the budget was consumed by requests that were not running. Reservations now carry `created_at`, and startup reclaims any older than `reservation_ttl_seconds`. The TTL is longer than any provider timeout, so a live request is never reclaimed. `reconciled_orphans` appears in the snapshot, on `/metrics` and in the dashboard.

## Client compatibility

The router implements the OpenAI **Chat Completions** API at `/v1/chat/completions`, streaming and non-streaming. `tools`, `tool_choice`, `functions`, `response_format`, `stop`, `temperature` and `seed` are forwarded unchanged; the router chooses the model, so the client's `model` is accepted and ignored. `GET /v1/models` lists `x3-auto` and `GET /v1/models/x3-auto` serves it, which is what clients probe before their first call.

It does **not** implement the Responses API. `POST /v1/responses`, `/v1/embeddings` and `/v1/audio/*` answer `501` naming the gap rather than `404`, so a client that needs them fails visibly instead of looking like a wrong base URL. An agent that requires the Responses API cannot use this router yet; that is the largest remaining compatibility gap.

## Operational notes

Open `/v1/dashboard` for a local spend dashboard or scrape `/metrics` for Prometheus. When `X3_ROUTER_TOKEN` is set, the dashboard accepts HTTP Basic username `x3` and that token as password; API clients can keep using bearer auth. Both views require authentication and show daily spend, reservations, and completed requests.

Streaming requests ask providers for a usage event. When usage is unavailable, the reserved estimate is charged. A provider may be retried before the first SSE event; a broken partial stream closes without switching models.

Still missing, in the order they matter for relying on this with X3 agents:

- **Verified escalation.** Fallback reacts to provider failures, not to patches that fail their checks. The feedback work records which patches pass, but nothing routes on it yet.
- **Privacy controls.** There is no per-task local-only / trusted-cloud / public-code route enforcement. `routes.routine` is a single global list.
- **Context retrieval.** No repo index or context packets; prompts carry their own context.
- **Operational security.** One router token, no per-agent credentials; the usage database is an unencrypted SQLite file.

The service does not persist prompts or credentials.

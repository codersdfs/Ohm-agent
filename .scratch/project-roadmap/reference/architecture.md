# Architecture reference

The 30,000-foot view of how Omega is put together. Read this when you
are about to make a change that touches more than one crate.

## Crates

| Crate | Path | Role |
|---|---|---|
| `omega` (binary) | `src-tauri/crates/omega-cli/` | CLI entry point — clap-derived, runs `main.rs` |
| `omega-core` | `src-tauri/crates/omega-core/` | AppState, pipeline (plan→build→review→gate), commands, TUI |
| `harness` | `src-tauri/crates/harness/` | Mechanized Gate: rules engine + pattern scoring |
| `entropy` | `src-tauri/crates/entropy/` | Drift scanner + auto-GC PR generator |
| `omega-table` | `src-tauri/crates/omega-table/` | `.otable` three-level loading (index→meta→content) + LRU |
| `providers` | `src-tauri/crates/providers/` | `LlmProvider` trait + 14 providers (5 native + OpenAI-compatible) |
| `memory` | `src-tauri/crates/memory/` | Hermes memory: session/project/user layers, SQLite + FTS5 + embeddings |
| `mcp` | `src-tauri/crates/mcp/` | MCP JSON-RPC client + skills registry |

Dependency chain: `omega` → `omega-core` → `{harness, memory, mcp,
omega-table, providers}`.

## The chat loop

A user message enters `omega-core::commands::chat::handle_chat`. The loop:

1. Assemble context (memory + repo-map + history).
2. Build the tool definitions (from `tool-harness::default_tool_registry` +
   `commands::mcp::tool_definitions` + `commands::agent_skills::load_skill`).
3. Call the LLM (streaming).
4. If the LLM emits tool calls, run each through `execute_tool_inner` (the
   shared pipeline with hooks).
5. Push tool results into the message history.
6. If the LLM emitted no tool calls, this is the final response — done.
7. Otherwise, goto 1 with the updated history.

`execute_tool_inner` is the single chokepoint. Every tool call — whether
from the user, a subagent, or a slash command — goes through it. The hooks
(Gate, permission, budget, observability) are registered once in the
`OnceLock` at the top of `execute_tool_inner` and run for every call.

## The Gate

The Gate is a `GateHook` registered into the tool pipeline. It runs on
every `write`, `edit`, and `apply_patch` call. The scorer is a `GateScorer`
closure that returns `(score, violations)`. The score is compared against
the pass threshold (default 80).

Three modes:

- `Block` — `HookDecision::Deny` if score < threshold; the write does not
  happen.
- `Warn` — `HookDecision::Inject` with the violations as advice; the
  write happens, the LLM sees the advice on the next turn.
- `AdviceOnly` — log only, never blocks, never injects.

The mode is read from `OMEGA_GATE_MODE` env var, defaulting to `Warn`.
This is a known footgun — new users hit the default and don't realize
the Gate is in warn mode. Future work: a clearer default for the first
session.

## Subagent delegation

`commands::chat::handle_spawn_subagent` is an inline intercept: when the
LLM emits a `spawn_subagent` tool call, this function is called instead
of the normal `execute_tool_inner`. The subagent runs `Subagent::run` from
`subagent::subagent::Subagent::run`, which:

1. Forks the parent context (Full / TaskScoped / CleanSlate).
2. Swaps in the subagent system prompt.
3. Enters its own loop: LLM call → tool call → executor → LLM call.

The subagent's `executor` callback calls `execute_tool_inner` (the same
shared pipeline). So subagent writes go through the parent's gate,
permission, and budget hooks. This is verified by the ticket #11 tests.

## Negative-knowledge loop (planned)

When a tool error has appeared >= 3 times (configurable), the error is
promoted to a linter rule in the Gate's rules DB. The new rule is loaded
on subsequent runs and blocks the same failure on the first try.

This is currently partial — frequency tracking exists, auto-promotion
does not. Phase 1 work.

## Provider abstraction

`providers::LlmProvider` is the trait. 14 implementations: 5 native
(Anthropic, Bedrock, OpenAI, Local Ollama, plus the OpenAI-compatible
fallback) and 9 via the OpenAI-compatible transport (XAI, Cerebras,
Groq, Kimi, MiniMax, OpenRouter, Azure, HuggingFace, Mistral).

Selection is by `ProviderKind` enum in the user's `config.json`. The
default is local Ollama at `http://127.0.0.1:11434` with model
`llama3.1:8b`. The user can switch at runtime via the TUI provider
panel (Ctrl+P).

## Tool definitions

The `tool-harness::default_tool_registry()` returns the 14 built-in
tools. `commands::mcp::tool_definitions()` extends this with tools from
connected MCP servers. `commands::agent_skills::load_skill` adds the
dynamic skill loader. The combined list is what the LLM sees.

## Where things live (file index)

- `src-tauri/crates/omega-cli/src/main.rs` — CLI entry.
- `src-tauri/crates/omega-core/src/lib.rs` — AppState + ChatEmitter trait.
- `src-tauri/crates/omega-core/src/commands/chat.rs` — chat loop,
  `handle_chat`, `handle_spawn_subagent`.
- `src-tauri/crates/omega-core/src/commands/tools.rs` —
  `execute_tool_inner`, `tool_definitions`, `default_system_prompt`.
- `src-tauri/crates/omega-core/src/gate_hook.rs` — `gate_hook_from_state`
  and the live Gate enforcement.
- `src-tauri/crates/omega-core/src/subagent/` — subagent code.
- `src-tauri/crates/harness/src/engine.rs` — Gate scoring rules engine.
- `src-tauri/crates/tool-harness/src/pipeline.rs` — `ExecutionPipeline`,
  the shared tool-execution chokepoint.
- `.github/workflows/release.yml` — release pipeline. (See
  `../release-pipeline-postmortem.md` for the ten fixes.)

## Performance notes (rough)

- Cold compile: 60-90 seconds for `cargo build --workspace`.
- Test suite: 547 tests in ~30 seconds (most are unit tests; the three
  subagent integration tests take 20s each because of the gate + mock
  setup).
- Release build: 5-10 minutes per platform.
- LLM call latency dominates the agent loop. Local Ollama is ~50ms/token,
  Anthropic is ~200ms/token. The Gate runs in microseconds and is not
  a bottleneck.

## What this document is NOT

- **Not an API reference.** The code is the API. For a specific function,
  read the doc-comment on the function.
- **Not a tutorial.** For "how do I add a new tool", see `tasks/` for
  past examples. (TODO: this should be a separate doc. Phase 1 work.)
- **Not exhaustive.** The 5% of the architecture that matters for the
  roadmap is above. The other 95% is in the code.

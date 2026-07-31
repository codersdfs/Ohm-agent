# Competition Route-Finding: Gap Analysis

## Current Reality of `src-tauri/` (as of assessment)

### Compiles & tests pass
| Crate | Status | Notes |
|-------|--------|-------|
| `harness` | ✅ 63 tests | Gate engine: structural/taste/golden/repeated + external linter wrappers (clippy/eslint/tsc/ruff) + tree-sitter complexity metrics |
| `tool-harness` | ✅ 65 tests | 14 built-in tools with rich metadata taxonomy (category, latency hint, cost hint, error spec) |
| `memory` | ✅ 11 tests | SQLite+FTS5 three-layer store; **embeddings are n-grams, not fastfeed/ONNX** (README says "fastembed") |
| `providers` | ✅ compiles | 14-provider enum + unified `LlmProvider` trait, streaming + non-streaming |
| `mcp-server` | ✅ compiles | HTTP JSON-RPC MCP server exposing built-in tools |
| `mcp` | ✅ compiles | HTTP-only JSON-RPC client (not stdio — ROADMAP P1-02 acknowledges this) |
| `omega-table` | ✅ compiles | `.otable` three-level progressive-load format |
| `taste` | ✅ compiles | Static rule-based checks, optional ML feature off by default |

### Broken / blocked
| Crate | Status | Issue |
|-------|--------|-------|
| `entropy` | ✅ compiles | `Language::detect()` now implemented in harness; `Language::label()` added for pipeline prompts. Drift scanner + GC now functional. (Fixed by ticket 006.) |
| `omega-core` | ✅ compiles | Depends on `entropy`; now builds. Includes pipeline + chat + commands + TUI rendering |
| `omega-cli` | ✅ compiles | Package name is `omega` not `omega-cli`; only `Chat` + `ServeMcp` subcommands exist (README advertises 10 more) |

### Fix applied
- **Ticket 006 (closed)**: Implemented `Language::detect(paths: &[String])` in `harness/src/language.rs` — manifest-file scan (Cargo.toml→Rust, package.json→TypeScript, etc.). Added `Language::label()` returning a human-readable label. Fixed corrupt `Cargo.lock` (truncated `anstyle` checksum). `cargo check -p omega-core -p omega` passes. 203 tests pass across entropy (5), harness (63), omega-core (135). Entropy GC `DriftScanner` + `GarbageCollector` now compile and can serve as a repo-wide Gate scan.

### Multi-agent pipeline state
- `PlanAgent` (`pipeline/plan.rs`): generates structured JSON plan via LLM — code exists, quarantined behind experimental env var.
- `BuildAgent` (`pipeline/build.rs`): executes plan steps via LLM-generated file content — **empty-file-safe** (rejects empty content), permission-gated.
- `ReviewAgent` (`pipeline/review.rs`): combined gate + LLM review with score aggregation.
- **None of these are wired into the CLI** — `CliAction` enum has only `Chat` and `ServeMcp`.

### Competitive claims vs reality
| README claim | Reality |
|---|---|
| "14 providers, zero lock-in" | Abstraction exists ✅, but no router health-check, no cost/latency failover wired in (P1-05). |
| "Outperforms other coding agents" | Unproven. No eval harness (P1-08 unstarted; only hand-written `baseline.md`). |
| "60-80% of violations caught by Gate alone" | Heuristics exist ✅ (tree-sitter + patterns + linter wrapping), but no measured FP rate in shipping code. |
| "Embedding-based semantic search (fastembed)" | n-gram 256-dim hash, not a real embedding model (P1-04). |
| "Plan/Build/Review subcommands" | Don't exist in CLI (P0-01 README rewrite ticket acknowledges). |
| "1M+ lines at OpenAI" | External Harness Engineering reference, not Omega result. |

### Tool gap vs competitors
| Feature | Omega | Claude Code | Codex CLI |
|---|---|---|---|
| Chat loop + tools | ✅ single-agent TUI | ✅ dual-mode IDE | ✅ single-agent |
| Built-in tools (read/write/edit/bash/grep/glob) | ✅ 14 tools | ✅ full VS Code extension API | ✅ full shell access |
| Native tool execution (zero spawn) | ✅ Rust-native | Shell calls | Shell calls |
| MCP support | ❌ client is HTTP-only stub | ✅ full MCP client + server | ❌ |
| Repo map / symbol index | ❌ not built (P1-03) | ✅ built-in | ✅ |
| Git workflow (commit/PR) | ✅ git_status/diff/log/commit tools | ✅ /commit /pr | ✅ `git diff` + apply |
| Permission modes | ❌ TUI auto-approves writes | ✅ configurable trust | ✅ read-only/full |
| Session persistence | ✅ SQLite session store | ✅ | ✅ |
| Multi-agent (plan→build→review) | Gated behind env var ❌ | ❌ single-agent | ❌ single-agent |
| Binary releases | ❌ not built (P1-07) | ✅ | ✅ |
| Eval harness | ❌ (P1-08) | ✅ SWE-bench-style | ✅ |

### The one real moat
The **Mechanized Gate** — deterministic Rust engine catching structural/taste/golden/
repeated violations + external linter results in microseconds, with a negative-knowledge
loop that promotes recurring errors to permanent rules. No competitor has anything
analogous. But it's **unmeasured** (no real FP-rate data, no eval showing gate-on
improvement) and the surrounding pipeline is gated/non-compiling.

## Competitive landscape summary
- **Claude Code**: ships, full IDE integration, MCP ecosystem, repo indexing,
  configurable trust, binary releases. Dominant.
- **Codex CLI**: ships, single-agent loop, shell-based, npm-installable, eval-backed.
  Simpler but functional.
- **Omega Agent**: better-architected moat (Gate + negative knowledge + multi-agent
  pipeline + 14-provider abstraction) but **not shipping end-to-end**. Cannot
  compile `cargo check --workspace`. Cannot run `cargo test -p omega-core`.
  README claims ahead of code by P2-04.

## The core strategic question
Is Omega's path to competitiveness:
(A) **Ship what works now** — ship just the chat TUI + Gate as a single-agent loop,
honest about being alpha, with real evals and a real README (the P0 roadmap)?
(B) **Bet on the moat** — invest through P1/P2 to make the Gate + multi-agent
pipeline + MCP client + real embeddings + repo map + binary releases all real,
then compete on quality/safety rather than feature-parity?

This map charts the decisions that determine which path, and what gates each path.

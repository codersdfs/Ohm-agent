# Ticket: Ship-Now vs Invest-in-Moat — Which Strategic Path?

## Question

Given the gap analysis (see PLAN_SUMMARY.md), Omega Agent faces a fork:

- **Path A (Ship-now):** Strip the overclaiming, fix the compile error, ship
  just the single-agent chat TUI + Gate as an honest alpha. Compete on
  "deterministic quality gate in Rust" as the one differentiator. Skip
  multi-agent pipeline, real MCP client, embeddings, binary releases for now.
  Target: working `cargo install` + eval harness showing gate improves quality.

- **Path B (Invest-in-moat):** Execute the full Phase 1 + Phase 2 ROADMAP —
  real MCP stdio client, repo map with tree-sitter indexing, real embeddings,
  provider routing with health checks, working multi-agent pipeline, binary
  releases, eval harness. Target: 60%+ multi-file task success, gate FP < 15%.

### Tradeoffs to weigh
| | Path A | Path B |
|---|---|---|
| Time to first competitive signal | 2-3 weeks | 6-12 months |
| Engineering effort | ~3 crates fixed + docs + evals | ~12 roadmap tickets, multiple crates |
| Risk | Low (just honesty + fix) | High (many moving parts, no guarantee moat wins) |
| Market positioning | "alpha agent with a Rust quality gate" | "full-featured AI coding agent" |
| Competitive threat | Claude Code will lap it on features | Claude Code may ship Gate-equivalent first |

### Decision criteria
- Which path maximizes the **probability that Omega becomes a viable daily-
  driver for at least some developers within 12 months**?
- Is the Gate moat strong enough to defend against Claude Code's vastly larger
  engineering team shipping faster?
- Does Path A risk "giving up" the multi-agent + 14-provider differentiation
  prematurely, or does Path B risk burning runway on architecture while
  Claude Code eats the market?

### Research needed
- Estimate engineering effort for Path B using ROADMAP P1-01 through P2-07
  (count crates touched, test requirements, dependency on external tools like
  `tree-sitter` grammars, `fastembed` model downloads).
- Estimate Path A effort (entropy fix, README rewrite, eval harness skeleton
  with 5 tasks, binary build script).
- Look at Aider's trajectory: when did it become competitive? What was its
  minimal feature set at v1.0?
- Market window: how fast is Claude Code iterating on MCP/hooks/quality gates?

## Resolution

### Answer: **Path A (Ship-Now) is the correct strategy.**

### Why Path A

**1. Aider's trajectory provides the decisive precedent.**
Aider — the most successful independent coding agent — followed exactly Path A's playbook:
- **v0.x era (2024)**: Started as a single-agent chat loop with repo map, edit-by-diff, git commit, and a weak "editor model" for cheap commits. Minimal CLI surface (`-m`, `--model`, edit-format options, `/help`). No multi-agent pipeline, no mechanical gate, no MCP until later.
- **v0.33-v0.50 (2025)**: Gained `/architect` mode (two-model split within a single agent loop, not separate agents), git operations, and a growing tool set. Still single-agent.
- **v0.77+** (2025-06): Only then did it add MCP server support and the repo-map-as-a-tool (`/context` command). Architect mode is described as "decouples high-level planning from precise editing" — it's a **prompt-routing strategy within one agent loop**, not separate Plan/Build/Review agents with state machine retries.

Aider did not ship a multi-agent Plan→Build→Review pipeline with a deterministic Gate between stages. Its competitive success came from **shipping a working single-agent loop early and iterating fast**, not from betting on architecture.

**2. Path B effort is 12-18 months and high-risk.**
Counting ROADMAP P1-01 through P2-07:
- **P1-01** (expand tools): 6 new tool modules — low effort, but the existing 14 tools are already implemented.
- **P1-02** (MCP stdio rewrite): Complete transport-layer rewrite of `mcp/src/transport.rs` from HTTP-only to stdio JSON-RPC — moderate effort, high complexity.
- **P1-03** (repo map + tree-sitter): New `repomap.rs` module, dependency on tree-sitter grammars (rust/typescript/python at minimum), caching layer — moderate effort.
- **P1-04** (real embeddings): Integrate `fastembed` or `ort` ONNX runtime, model download management, fallback logic — high effort (model weight management, cross-platform binary issues).
- **P1-05** (permission modes): Full permission matrix implementation across chat loop + TUI — moderate effort.
- **P1-06** (git/PR workflow): git tools already exist (git_status, git_diff, git_log, git_commit), but `gh pr create` integration is new — low effort.
- **P1-07** (binary releases): CI workflow for 3 platforms — low effort.
- **P1-08** (eval harness): 20 tasks + runner + baseline — moderate effort.
- **P1-09** (VS Code extension): Minimal — low effort.
- **P2-01** (Gate v2 wraps real linters): Integration of clippy/eslint/tsc/ruff — moderate effort.
- **P2-02** (tree-sitter structural metrics): AST parsing for function length/complexity — moderate-high effort (grammar integration).
- **P2-03** (Negative Knowledge Loop v2): Full failure-logging → rule-promotion → system-prompt injection loop — moderate effort.
- **P2-04** (multi-agent pipeline that works): Full reimplementation of PlanAgent/BuildAgent/ReviewAgent with real tool calls, per-role model routing, delta retry — **high effort, highest risk**.
- **P2-05** (provider router): Wire router.rs into the CLI, health checks, failover logic — moderate effort.
- **P2-06** (Entropy GC real MVP): Currently compiles but only supports Rust (rustfmt/clippy), needs multi-language support — moderate effort.
- **P2-07** (case study): Documentation — low effort.

Total: ~8-10 medium-to-high complexity tickets across 6-12 months, touching 4+ crates (harness, providers, mcp, memory, omega-core), with external dependencies (tree-sitter grammars, ONNX models, Docker for sandboxes). **Risk**: Claude Code iterates weekly on MCP/hooks/quality gates. If Omega spends 12 months on the full moat, it ships into a market where Claude Code already has hooks + sandboxing + MCP + repo indexing + a 10x larger engineering team.

**3. Market window analysis.**
- Claude Code's hooks system (https://code.claude.com/docs/en/hooks) is **opt-in project shell scripts**, not a default-on Rust gate. This is Omega's window: ship a **default-on deterministic Gate** before Claude Code makes hooks first-class.
- But Claude Code iterates fast: weekly releases, full MCP ecosystem, VS Code IDE integration. The differentiation window for "quality gate" is narrow if not shipped quickly.
- Codex CLI is single-agent, shell-based, npm-installable, eval-backed. It proves that **single-agent + good docs + evals > complex architecture** for market adoption.

**4. Path A effort is 3-4 weeks and low-risk.**
- Fix entropy compile (already done, ticket 006) ✓
- Honest README rewrite (P0-01 equivalent to ticket 004) — 2 days
- Eval harness skeleton with 5 tasks — 3 days
- Binary build script (cargo build + GitHub release) — 1 day
- Wire Gate into the existing chat tool loop (already partially done via `execute_tool_inner` gate checks) — 1 day
- Ship `cargo install` with a working single-agent TUI + Gate

### The hybrid strategy

The honest conclusion is **not** "Path A vs Path B" as a binary choice — it's **Path A first, Path B gated behind experiments**:

1. **Ship Path A immediately**: single-agent chat + Gate as alpha, honest README, eval harness showing gate improves quality, binary releases. Target: 2-3 weeks.
2. **Keep multi-agent pipeline behind `OMEGA_EXPERIMENTAL_PIPELINE=1`**: it already exists, it already compiles (after ticket 006 fix), it already has the empty-file-safe guard. Do not delete it — let it mature as an experimental feature while the single-agent loop ships.
3. **Invest in the Gate moat incrementally**: P2-01 (wrap real linters) and P2-03 (negative knowledge loop) both make the Gate stronger for the *single-agent* loop too — they are not pipeline-exclusive.
4. **Defer Path B "big bets"**: real MCP stdio client (P1-02), real embeddings (P1-04), repo map (P1-03), and the full pipeline (P2-04) — these are Phase 2 work that can come after the single-agent loop is proven.

This mirrors Aider's actual trajectory: ship the minimum viable agent, then add MCP, architect mode, and advanced features as incremental upgrades — not as a big-bang rewrite.

### Effort verification

**Path A verified against code:**
- The chat agent loop (`stream_message_with_history_cancel` in `chat.rs`) is the canonical interactive path and already integrates Gate checks via `execute_tool_inner` (line 167 of `commands/tools.rs`: gate runs on write/edit). ✅
- Session persistence (`session.rs`) already exists and is wired into the TUI. ✅
- Cancel/interrupt (`Arc<AtomicBool>`) is already threaded through the loop. ✅
- Context compaction (`context.rs`) already exists. ✅
- Tool truncation (`budget.rs` / output caps) already exists. ✅
- Permission modes (`check_permission` in `chat.rs`) exist but TUI auto-approves in `on` mode — fix needed (P1-05). ⚠️
- Only 2 CLI subcommands exist (`Chat`, `ServeMcp`) — the README's 10-advertised subcommands don't compile. ✅ (needs README fix)

**Path B verified against code:**
- `mcp/src/transport.rs`: HTTP-only JSON-RPC, no stdio transport. ❌
- `memory/src/embed.rs`: n-gram 256-dim hash, ONNX engine behind feature gate but not default. ❌ (not real embeddings)
- `providers/src/router.rs`: `route_request()` exists but is **not wired into any CLI command** — `omega-cli/src/main.rs` uses flat `load_provider_config()`. ❌
- `pipeline/build.rs`: `step_to_tool_request` now generates non-empty content via LLM (P0-02 fix applied), but pipeline is `OMEGA_EXPERIMENTAL_PIPELINE=1`-gated and not in CLI. ⚠️
- No eval harness in `evals/` — only `baseline.md` mentioned. ❌
- No repo map / symbol index — `repomap.rs` doesn't exist. ❌

### Decision

**Path A (ship-now) is the clear winner.** The single-agent chat + Gate loop is already substantially working (chat.rs, tools.rs, session.rs all functional). The honest README + eval harness + binary release can ship this in 2-3 weeks. Multi-agent pipeline, real MCP, real embeddings, provider routing, and repo map are Path B features to deliver after the single-agent loop is proven in the market.

The Gate moat remains viable under Path A: the deterministic Rust gate already runs on every write/edit in the chat loop. Making it stronger (P2-01, P2-02, P2-03) improves the single-agent loop too — those are incremental investments, not blockers.

## Type: grilling

## Status: closed

## Assigned to: omega-wayfinder

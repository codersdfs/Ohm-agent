# Omega Agent

**Multi-agent AI coding assistant** — orchestrates Plan, Build, and Code Review agents through a Rust backend with Mechanized Gate enforcement, entropy garbage collection, negative knowledge feedback, and structured table memory.

Built on the principles of [Harness Engineering](https://github.com/anomalyco/harness-engineering) — the framework that enabled 3 engineers to ship 1M+ lines of production code in 5 months using AI.

---

## Why Omega Agent?

Most coding agents generate once and hope for the best. Omega Agent **plans, builds, reviews, gates, scores, and retries** — low-quality output never reaches your repository.

| Problem | Omega's Solution |
|---------|------------------|
| LLMs miss structural issues | **Deterministic Gate in Rust** — catches 60-80% of violations in microseconds, no LLM tokens spent |
| Single model for everything | **Three specialized agents** — Plan (reasoning), Build (implementation), Review (critique) |
| Same mistakes repeat | **Negative knowledge loop** — errors at frequency ≥ 3 auto-promote to linter rules |
| Context pollution | **Hermes memory** — FTS5 + embedding retrieval, not wholesale context dumps |
| Provider lock-in | **14 providers, zero lock-in** — route to Groq for speed, Opus for review, local for privacy |
| No learning mechanism | **Entropy GC** — daily drift scans, auto-generates remediation PRs |

---

## Architecture

```
┌───────────────────────────────────────────────────────────────────┐
│                      Omega CLI                                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                         │
│  │   Plan    │  │   Build   │  │  Review   │                      │
│  │  Agent    │  │   Agent   │  │   Agent   │                      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                         │
│       │              │              │                             │
│  ┌────┴──────────────┴──────────────┴────┐                        │
│  │         Pipeline State Machine         │  max 3 retries        │
│  │     (omega-core/pipeline/)             │  pass threshold ≥ 80  │
│  └────────────────┬───────────────────────┘                       │
│                   │                                               │
│  ┌────────────────┴───────────────────────┐                       │
│  │       Mechanized Gate (harness/)        │                      │
│  │  structural • taste • golden • repeated │                      │
│  └────────────────┬───────────────────────┘                       │
│                   │                                               │
│  ┌────────────────┴───────────────────────┐                       │
│  │         Negative Patterns               │                      │
│  │   frequency ≥ 3 → auto-promote to rule  │                      │
│  └────────────────────────────────────────┘                       │
│                                                                   │
│  ┌────────┐ ┌─────────┐ ┌──────────┐ ┌──────┐ ┌───────┐           │
│  │Providers│ │ Omega   │ │ Hermes   │ │Entropy│ │ MCP   │         │
│  │(14 LLM) │ │ Tables  │ │ Memory   │ │  GC   │ │ Skills│         │
│  └────────┘ └─────────┘ └──────────┘ └──────┘ └───────┘           │
└───────────────────────────────────────────────────────────────────┘
```

---

## Workspace Crates

| Crate | Path | Purpose |
|-------|------|---------|
| `omega` | `src-tauri/crates/omega-cli/` | CLI binary — clap-based entry point with all subcommands |
| `omega-core` | `src-tauri/crates/omega-core/` | Core library — `AppState`, pipeline orchestration, commands, TUI markdown rendering |
| `harness` | `src-tauri/crates/harness/` | **Mechanized Gate** — rules engine, pattern matching, scoring |
| `entropy` | `src-tauri/crates/entropy/` | Drift scanner, domain scorer, auto-GC PR generation |
| `omega-table` | `src-tauri/crates/omega-table/` | `.otable` format — three-level loading (index → meta → content), LRU cache |
| `providers` | `src-tauri/crates/providers/` | LLM abstraction — 14 providers via unified `LlmProvider` trait |
| `memory` | `src-tauri/crates/memory/` | **Hermes Memory** — session/project/user layers, SQLite + FTS5 + embeddings |
| `mcp` | `src-tauri/crates/mcp/` | MCP client — JSON-RPC transport, skills registry |

---

## The Three Agents

### Plan Agent (Read-Only)
- **No tool access** — pure reasoning, no permission dialogs
- Produces structured plans in `.otable` format with atomic steps, dependencies, and line estimates
- Uses your configured reasoning model (default: Claude Sonnet)

### Build Agent (Write Access)
- Executes plans via filesystem, bash, grep, glob — native Rust calls, zero spawn overhead
- Requests permission via frontend dialog before mutating files
- Uses your configured implementation model (default: Claude Sonnet)

### Code Review Agent (Read-Only, Strongest Critique)
- Reviews output against golden rules, structural/taste patterns
- Every violation includes an **executable tool call** — the agent can fix it immediately
- Uses your strongest configured model (default: Claude Opus)

---

## Pipeline

```
Plan ──→ Build ──→ Review ──→ Gate ──→ Score ≥ 80? ──→ Done
                  ↑                              │
                  └──────── max 3 retries ────────┘
```

**Scoring**: 100 base − 15 (structural) − 10 (taste) − 20 (golden) − 25 (repeated)  
**Pass threshold**: ≥ 80  
**Delta retry**: Only the diff is re-sent — no full replan  
**Context cache**: Cached until `.omega/` files change

---

## Core Concepts

### Harness Engineering (OpenAI / Ryan Lopopolo)

1. **Repo as System of Record** — Everything outside the repo is invisible. Slack, docs, tribal knowledge → versioned artifacts.
2. **Map, Not Manual** — `AGENTS.md` ≈ directory page (~100 lines), not encyclopedia lines). Progressive disclosure.
3. **Mechanical Enforcement** — Docs rot, lint rules don't. Custom linter + CI = invariant guardians. Errors embed fix instructions for self-correction.
4. **Agent Readability** — Boring tech (stable APIs, good training coverage). Reimplement subsets rather than wrap opaque upstreams.
5. **Entropy & GC** — Agents replicate patterns (including bad ones). Golden rules encoded in repo. Scheduled scans remediate drift.
6. **Humans Steer, Agents Execute** — Scarcest resource is human attention. Problem → missing context/tool/constraint, not "try harder".

### Guides × Sensors Matrix (Fowler / Böckeler, 2026)

| | Computational (CPU) | Reasoning (LLM) |
|---|---|---|
| **Guides / Feedforward** | Bootstrap scripts, OpenRewrite, LSP | `AGENTS.md`, Skills, architecture docs |
| **Sensors / Feedback** | Linter, ArchUnit, type checks, coverage | AI code review, LLM-as-judge |

### 6D Complexity Framework (Harness Engineering)

| Dimension | Focus |
|-----------|-------|
| D1: Structural | Architecture layering, dependency direction |
| D2: Taste | Code conventions, naming, file size limits |
| D3: Golden | Non-negotiable quality invariants |
| D4: Repeated | Frequency ≥ 3 → auto-promote to linter rule |
| D5: Context | Context window optimization, compaction |
| D6: Drift | Entropy scan, GC PR generation |

### Negative Knowledge Loop (Ralph Wiggum / Control Theory)

```
Error → Log → Count ≥ 3 → Promote to rule → Never happens again
```

Every error is logged. At frequency ≥ 3, it becomes a permanent linter rule. The system literally gets smarter over time.

---

## LLM Providers (14, Zero Lock-In)

| Provider | Transport | Best For |
|----------|-----------|----------|
| Anthropic | Native SDK | Strongest reasoning (Opus) |
| OpenAI | Native SDK | General purpose |
| Google (Gemini) | Native SDK | Large context |
| Mistral | Native SDK | EU data residency |
| xAI (Grok) | OpenAI-compatible | Speed |
| Cerebras | OpenAI-compatible | Ultra-fast inference |
| Azure OpenAI | OpenAI-compatible | Enterprise |
| AWS Bedrock | OpenAI-compatible | Enterprise |
| Hugging Face | OpenAI-compatible | Open models |
| Groq | OpenAI-compatible | **Fastest** — simple checks |
| Kimi for Coding | OpenAI-compatible | Coding specialized |
| MiniMax | OpenAI-compatible | Long context |
| OpenRouter | OpenAI-compatible | Model routing |
| Local / Custom | OpenAI-compatible endpoint | **Privacy** — fully offline |

8 providers share OpenAI-compatible transport (~1,050 lines total). Route simple lint-style checks to Groq (fastest) and complex reasoning to Opus (strongest).

---

## Omega Tables (`.otable`)

Three-level progressive loading — only load what the agent actually needs:

```
.otable file
├── Level 1: Index  (schema, columns, row count, version)
├── Level 2: Meta   (description, tags, source, stats)
└── Level 3: Content (actual rows, paginated)
```

- **LRU cache with TTL eviction**
- **FTS5 full-text search** (via SQLite)
- **Embedding-based semantic search** (via fastembed)

---

## Hermes Memory

Three-layer memory system with automatic context injection:

| Layer | Scope | Persistence |
|-------|-------|-------------|
| Session | Current session | In-memory, cleared on exit |
| Project | Current project | SQLite per-project database |
| User | Cross-project | SQLite user-wide database |

- **FTS5** for full-text search
- **Embedding vectors** for semantic similarity
- **Automatic context injection** into agent prompts — relevant past context retrieved, not dumped wholesale

---

## Entropy GC

- Runs daily as a scheduled background task
- Scans domains for structural drift
- Scores each domain by drift severity and priority
- **Auto-generates PRs** to remediate high-entropy areas

---

## Token Efficiency — Why Omega Wastes Fewer Tokens

| Mechanism | How It Saves Tokens |
|-----------|---------------------|
| **Gate-first review** | Catches structural/taste violations deterministically in Rust (µs) — no LLM tokens re-identifying the same issues |
| **Delta retry** | Only the diff is re-sent, not full plan + build context |
| **Context cache** | Unchanged context skips re-embedding and re-tokenization entirely |
| **Three-level `.otable`** | Progressive loading: index → meta → content — load only what's needed |
| **Progressive disclosure** | `AGENTS.md` is a ~100-line directory; deeper docs loaded on demand |
| **Negative knowledge loop** | Errors at frequency ≥ 3 become linter rules — never pay for the same mistake twice |
| **Plan agent is read-only** | No tool-call overhead for permission dialogs; pure reasoning |
| **Skills registry (MCP)** | Tools loaded on demand, not pre-loaded at startup |
| **Hermes memory** | Relevant context retrieved via FTS5 + embedding search, not dumped wholesale |

---

## Performance — Why Omega Is Faster

| Factor | Why |
|--------|-----|
| **Deterministic gate in Rust** | Gate checks run in µs — LLM review takes seconds. Gate catches 60-80% alone |
| **Parallel agent pipeline** | Plan finishes before Build starts (sequential by design), but Review + Gate run in near-parallel |
| **Route to fastest provider** | 14 providers — dispatch to Groq for speed, Opus for review, local for privacy |
| **Context cache hits** | Unchanged context skips re-tokenization — critical for large projects |
| **Delta-only retries** | Smaller prompt → faster LLM response per retry |
| **Rust-native tool execution** | Filesystem read/write/bash/grep/glob as native calls — zero spawn overhead |
| **MCP skill loading** | Only load skills needed for the current task |

---

## Why Omega Outperforms Other Coding Agents

### 1. Separation of Concerns (Three Specialized Agents)
Most agents use a single model for everything. Omega splits the work:
- **Plan** — read-only, pure reasoning, no tool distractions
- **Build** — write access, focused on implementation  
- **Review** — strongest model used purely for critique, not generation

Each model does what it's best at. Context windows never polluted by other agents' concerns.

### 2. The Gate Is Independent of the LLM
The Mechanized Gate is a **deterministic Rust engine** enforcing structural, taste, golden, and repeated-error rules. It catches what LLMs consistently miss:
- LLMs are bad at counting lines, checking file sizes, verifying import paths
- The Gate never hallucinates, never forgets, never gets tired
- Every violation includes an executable tool call — immediate self-correction

### 3. Self-Improving (Negative Knowledge Loop)
Every error logged. At frequency ≥ 3 → permanent linter rule:
```
Error → Log → Count ≥ 3 → Promote to rule → Never happens again
```
No other coding agent has a closed-loop learning mechanism like this.

### 4. Battle-Tested Philosophy
Built on **Harness Engineering** — the framework that enabled **3 engineers to build 1M+ lines of production code in 5 months** using AI (zero hand-written code). The six core concepts are not theoretical — they produced measurable results at OpenAI scale.

### 5. The Scoring Loop Prevents Bad Code
```
Score = 100 − 15(structural) − 10(taste) − 20(golden) − 25(repeated)
Pass ≥ 80, max 3 retries
```
Most agents generate once and deliver. Omega scores, gates, and retries — low-quality output never reaches the repository.

### 6. Guides × Sensors = Full Control Loop
Most agents only have sensors (code review). Omega has both:
- **Guides**: `AGENTS.md`, Skills, architecture docs — increase first-attempt success
- **Sensors**: Gate, Review LLM, CI hooks — catch what guides missed

This is Martin Fowler's 2026 control-theory insight: guides alone mean you never know if they work; sensors alone mean you make the same mistakes repeatedly.

### 7. 14 Providers, Zero Lock-In
Not dependent on any single LLM provider. If one is down, slow, or expensive — route to another. Provider abstraction is ~1,050 lines of shared OpenAI-compatible transport.

---

## Development

### Prerequisites
- **Rust 1.77+** (nightly recommended)
- **Node.js 20+**
- Windows (primary) / macOS / Linux

### Setup

```bash
# Build the omega CLI
cargo build -p omega

# Run the omega CLI (defaults to REPL)
cargo run -p omega

# Or use the omega wrapper script
omega --help
```

### Project Structure

```
omega-agent/
├── src-tauri/                    # Rust workspace root
│   └── crates/
│       ├── harness/              # Mechanized Gate
│       ├── entropy/              # Entropy GC
│       ├── omega-table/          # Omega Tables
│       ├── providers/            # 14 LLM providers
│       ├── memory/               # Hermes Memory
│       ├── mcp/                  # MCP client + skills
│       ├── omega-core/           # Core: AppState, pipeline, commands
│       └── omega-cli/            # CLI binary ("omega")
├── Cargo.toml                    # Workspace root
└── AGENTS.md                     # Agent notes (gitignored)
```

### Commands

```bash
cargo build -p omega          # Build CLI binary
cargo run -p omega            # Run CLI (REPL default)
cargo check --workspace       # Type-check all crates
cargo test -p <crate>         # Test individual crate
cargo test -p <crate> -- <test_name>  # Single test
```

**Omega subcommands**: `chat`, `plan`, `code`, `plan-status`, `plan-approve`, `build`, `review`, `gate`, `memory`, `config`, `provider`, `models`, `repl` (default)

---

## Configuration

- **Default**: local Ollama at `http://127.0.0.1:11434` with model `llama3.1:8b`
- **Config**: `~/.config/omega-agent/config.json` (via `directories` crate)
- **API key**: `OMEGA_API_KEY` env var, `~/.config/omega-agent/.env` file, or interactive `omega provider` setup

---

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Rust over Python backend | Same language as harness; better perf for filesystem ops |
| All 3 agents LLM-reasoning | Flexibility over raw speed |
| Pipeline in Rust | Harness enforcement must run in-process |
| MCP via Rust JSON-RPC | MCP SDK is TypeScript-native; Rust impl needed |
| Local embeddings via fastembed | No external API calls; fully offline |
| SQLite + FTS5 for memory | Zero-config, embeddable, proven at scale |
| `.otable` custom format | Progressive loading + LRU beats full-load alternatives |

---

## License

MIT

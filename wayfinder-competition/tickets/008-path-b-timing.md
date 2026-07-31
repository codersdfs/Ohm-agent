# Ticket: Is Path B Timing Viable Before Losing Competitive Window?

## Question

Given that Path A can ship in 2-3 weeks and Path B requires 6-12 months of execution before competitive metrics are validated, does the hybrid strategy provide enough time to validate Path B components before Claude Code or competitors narrow the window? What is the timeline risk assessment?

## Timeline Analysis

### Current State (Day 0)
- Single-agent chat TUI + Gate works ✅
- README overclaims ❌ (ticket 004 addresses this)
- Entropy compile fixed ✅ (ticket 006 done)
- Multi-agent pipeline experimental only ⚠️
- No real MCP stdio client ❌
- No tree-sitter repo map ❌
- No real embeddings ❌

### Path Alpha (Path A) — Day 21 Target
| Milestone | Effort | Owner |
|-----------|--------|-------|
| Honest README rewrite | 2 days | Tech writer + dev |
| Eval harness skeleton (5 tasks) | 3 days | Researcher |
| Binary build script (CI) | 1 day | Dev |
| Gate into chat loop integration | 1 day | Dev |
| Testing & documentation | 4 days | Team |
| **Release `cargo install omega`** | **Day 11-21** | Release manager |

### Path B Component Validation Timeline (Parallel Work)

#### Phase 1 (Months 1-3)
| Task | Dependency | Duration | Success Criteria |
|------|------------|----------|-----------------|
| P1-02: MCP stdio rewrite | None | 4 weeks | Basic tool calls via stdin/stdout |
| P1-03: Repo map + tree-sitter | P1-02 (for tooling) | 6 weeks | Query projects, file contents |
| P1-08: Eval harness skeleton | None (in parallel with alpha) | 3 weeks | 20-task suite, runner framework |

#### Phase 2 (Months 4-9)
| Task | Dependency | Duration | Success Criteria |
|------|------------|----------|-----------------|
| P1-04: Real embeddings | P1-03 (repo needs embeddings) | 8 weeks | Working fastembed/ONNX integration |
| P1-05: Permission modes | Core CLI stable | 4 weeks | Fine-grained access control |
| P1-07: Binary releases | All crates stable | 2 weeks | Build CI for 3 platforms |
| P2-01: Gate v2 (real linters) | Gate working in alpha | 6 weeks | Full clippy/eslint/tsc integration |
| P2-02: Multi-agent pipeline production | P1-02, P1-03, P2-01 | 12 weeks | Stable Plan→Build→Gate→Review→Fix |
| P2-03: Provider routing | P1-02 (MCP) | 4 weeks | Dynamic model selection |
| P2-04: Context cache | P1-03 (repo map) | 4 weeks | LRU + FTS5 hybrid recall |
| P2-05: Delta retry | P2-02 (pipeline) | 3 weeks | File-level patch retries |
| P2-06: VS Code extension | Core stable | 4 weeks | Working TS plugin |
| P2-07: Advanced evals | P1-08 (framework) | 4 weeks | Benchmark against Claude Code |

## Competitive Threat Assessment

### Claude Code Trajectory (Based on public release cadence)
- Weekly releases historically → ~52 features/year
- Adding hooks system → could integrate quality checks within 3-6 months
- MCP ecosystem already exists → adding "quality gate" as a hook is plausible within 6 months
- VS Code IDE integration is solid → would compete directly on editor experience

### Codex / Copilot Trajectory
- Already deeply embedded in developer workflows
- GitHub-native positioning makes switching costly
- Any quality differentiation must be demonstrably better than GitHub's built-in checks

## Critical Path Analysis

The longest path through Phase 1 + Phase 2 is:
```
P1-02 (MCP stdio: 4w) → P2-02 (Multi-agent pipeline: 12w) = 16 weeks minimum
                   ↗
P1-03 (Repo map: 6w) ──┘
                    ↗
P2-01 (Gate v2: 6w) ──┴──▶ P2-02 requires both P1-02 + P2-01
```

**Earliest complete Path B execution**: ~22 weeks (5.5 months) if all goes perfectly, accounting for dependencies and integration overhead. With blockers, testing delays, and refactoring, realistically 8-12 months.

## The "Hybrid Strategy" Advantage

By shipping Path A alpha at Day 21:
- Establishes market presence before Claude Code can respond with feature parity
- Gains early user feedback for the Gate quality metric validation
- Creates revenue/crowdfunding potential to fund Path B development
- Provides a concrete benchmark for Path B improvements (e.g., "multi-agent improves success by X%")

But there's also the risk of being perceived as "just an alpha" while competitors add similar quality gates.

## Decision Points Before Full Path B Commitment

1. **Month 2 checkpoint**: Does MCP stdio (P1-02) work reliably enough to unblock multi-agent pipeline? If not, pivot to simplified architecture.
2. **Month 3 checkpoint**: Do early eval results from P1-08 show meaningful differentiator vs single-agent baseline? If no, multi-agent may not justify complexity.
3. **Month 5 checkpoint**: Are embedding-based retrieval actually useful for code context? If tree-sitter alone suffices, skip complex ML stack.
4. **Month 8 checkpoint**: Can multi-agent pipeline achieve ≥60% multi-file task success as per roadmap north-star metric? If not substantially better than single-agent, reconsider investment.

## Conclusion

The hybrid strategy provides critical runway (~6 months) to validate key Path B components before committing full resources. The competitive window for "deterministic quality gate" is narrowing as Claude Code evolves its hooks system, but Alpha release establishes first-mover advantage in *integrated* deterministic checking rather than opt-in scripts. If Path B component validation shows meaningful improvement over Path A foundation by Month 3-5, proceed with full execution; otherwise, pivot to focused incremental enhancement of Path A.

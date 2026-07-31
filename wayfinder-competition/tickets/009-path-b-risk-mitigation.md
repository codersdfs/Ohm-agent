# Ticket: What Are Critical Risks of Path B Execution and How to Mitigate Them?

## Question

Executing the full Phase 1 + Phase 2 ROADMAP (Path B) carries significant technical, scheduling, and market risks. What are the critical risk factors, and what mitigation strategies should be baked into the execution plan to reduce probability of failure or catastrophic delay?

## Risk Assessment Matrix

### High Probability / High Impact Risks

| # | Risk Category | Specific Risk | Probability | Impact | Mitigation Strategy |
|---|---------------|---------------|-------------|--------|---------------------|
| R1 | Technical | Tree-sitter grammar maintenance burden | Medium | High | Maintain community-sourced grammars locally; prioritize Rust/TS/Python first; add grammar version pinning to CI |
| R2 | Technical | ONNX Runtime cross-platform compatibility | Medium | High | Test on all target platforms early; include fallback to CPU-only builds; document known issues clearly |
| R3 | Scheduling | Multi-agent pipeline complexity exceeds estimates | High | High | Use spike prototypes for P2-02; limit scope to Plan→Build→Gate (minimum viable pipeline); defer Review/Fix to later iteration |
| R4 | Market | Claude Code integrates equivalent gate features within 6 months | High | High | Ship Path A alpha immediately; document unique advantages (zero-token, in-process); focus marketing on differentiator |
| R5 | Resource | Team bandwidth insufficient for parallel Path A + Path B development | Medium | High | Prioritize Path A core features; defer non-critical Path B tasks (VS Code extension, advanced evals) to after alpha launch |

### Medium Probability / High Impact Risks

| # | Risk Category | Specific Risk | Probability | Impact | Mitigation Strategy |
|---|---------------|---------------|-------------|--------|---------------------|
| R6 | Technical | Memory bloat from repo map + embeddings + three-layer store | Medium | High | Implement progressive loading limits; add memory usage metrics with alerts; provide `--disable-embeddings` CLI flag |
| R7 | Technical | MCP stdio transport reliability | Medium | High | Add comprehensive error handling and recovery; implement message ID tracking; write integration tests for process lifecycles |
| R8 | Technical | CI/CD matrix increases build time significantly | Medium | Medium | Use matrix optimization (only build on PR to main); leverage caching; consider containerized build agents |

### Low Probability / High Impact Risks

| # | Risk Category | Specific Risk | Probability | Impact | Mitigation Strategy |
|---|---------------|---------------|-------------|--------|---------------------|
| R9 | Supply Chain | tree-sitter or ONNX repository changes break builds | Low | High | Pin dependencies explicitly; monitor security advisories; maintain local vendored copies of critical assets |
| R10 | Team Key Person Dependency | Single developer owns complex subsystems (e.g., pipeline state machine) | Low | High | Document architecture decisions; pair programming on critical paths; rotate maintenance ownership quarterly |

## Contingency Plans for Path B Failure Modes

### If Multi-Agent Pipeline Fails Performance Tests (Month 8)
- **Pivot**: Reduce pipeline to Plan→Build→Gate (skip Review/Fix), keeping Gate as differentiator
- **Or Pivot**: Defer multi-agent entirely; enhance single-agent with better tool routing instead
- **Documentation**: Clearly communicate that multi-agent is "advanced feature" not core product

If Repo Map + Tree-Sitter Proves Unmaintainable:
- **Pivot**: Simplify to file-based indexing without full tree-sitter parsing; use heuristics for file type detection
- **Or Pivot**: Leverage existing editor language server capabilities instead of duplicating them

If Embedding Quality Doesn't Improve Recall Meaningfully:
- **Pivot**: Fall back to keyword + AST-based retrieval in memory store
- **Defer**: Postpone embedding integration until proven necessary by user feedback

## Governance and Decision Gates

To avoid sunk-cost fallacy, establish formal go/no-go gates before advancing each phase:

```
Phase 0 Completion → Gate 0.1: Path A Alpha Release
     │
     ▼
Phase 1.1 (MCP stdio) → Gate 1.1: Can we call tools via stdio reliably?
     │
     ▼
Phase 1.2 (Repo map) → Gate 1.2: Does repo map improve code understanding measurably?
     │
     ▼
Phase 1.3 (Embeddings) → Gate 1.3: Do embeddings provide value beyond repo map alone?
     │
     ▼
Phase 2.1 (Gate v2) → Gate 2.1: Does enhanced Gate measurably reduce violations?
     │
     ▼
Phase 2.2 (Multi-agent) → Gate 2.2: Does multi-agent achieve ≥60% multi-file success vs baseline?
     │
     ▼
Phase 2.3-2.7 (Remaining) → Production Release
```

At each gate, if the answer is "no," reconsider the entire investment before proceeding.

## Budget and Timeline Buffers

Given the optimistic nature of technical estimates for complex integrations:

- Add **30% buffer** to all duration estimates in official schedule
- Reserve **20% engineering capacity** for unforeseen technical debt work
- Establish **monthly financial checkpoints** against burn rate if funded externally
- Define **hard stop criteria**: e.g., if Month 3 checkpoint Gate 1.2 fails to demonstrate improvement over baseline, execute contingency plan

## Conclusion

The most critical risk is investing heavily in Path B components (especially multi-agent pipeline) without validating their competitive advantage at incremental checkpoints. The hybrid strategy with Path A alpha release provides an essential safety valve—early market validation while de-risking larger investments through gated progression. Formalize the go/no-go gates, define clear pivot options for each major component, and maintain capacity buffers to absorb technical surprises without derailing the entire roadmap.

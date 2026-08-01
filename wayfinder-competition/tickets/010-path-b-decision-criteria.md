# Ticket: What Are the Go/No-Go Decision Criteria for Committing to Full Path B Execution?

## Question

The hybrid strategy from ticket 002 recommends shipping Path A alpha first, then evaluating whether to commit to full Path B execution. What specific, measurable decision criteria should be established at each checkpoint before advancing to the next Phase 1 or Phase 2 milestone? When does the team say "proceed with full Path B" versus "pivot or abandon"?

## Decision Framework Overview

The commitment to full Path B is not a single yes/no decision but a series of progressive gates based on empirical validation at each phase. Each gate answers: **Does this component provide sufficient competitive justification to justify the engineering investment required for the next layer?**

## Gate Criteria by Phase

### Gate 0.1 — After Path Alpha Release (Day 21)

**Decision**: Proceed to Phase 1 development?

| Metric | Threshold | Measurement Method |
|--------|-----------|-------------------|
| Installation count | ≥50 `cargo install` attempts from public registry | GitHub releases + crates.io analytics |
| Early user feedback on Gate | ≥70% report usefulness in survey (N≥30 users) | User feedback form / Discord poll |
| Bug rate (alpha) | ≤2 critical bugs per week reported via GitHub issues | Issue tracker metrics |
| Documentation completeness | README accuracy = 100% (verified against code) | Automated test comparing claims vs code |

**Go/No-Go**: If all thresholds met → proceed to Phase 1 with funding/resource allocation. If not, fix alpha issues before proceeding; do not expand scope while core foundation is unstable.

---

### Gate 1.1 — After MCP Stdio Completion (Month 2)

**Decision**: Can multi-agent pipeline rely on reliable stdio transport?

| Metric | Threshold | Measurement Method |
|--------|-----------|-------------------|
| Stdio round-trip latency | <200ms average (p95 <500ms) | Microbenchmarks across OS platforms |
| Error recovery rate | ≥99% of recoverable failures auto-recovered | Integration test suite with injected failures |
| Tool call reliability | ≥98% success rate on repeated calls | Stress test with 1000+ sequential tool calls |
| Process lifecycle management | No orphaned processes after crashes | System monitoring during chaos experiments |

**Status**: ✅ ALREADY IMPLEMENTED. `mcp/src/stdio.rs` implements Content-Length framed transport with `StdioTransport::spawn()`. Integration test (`mcp/tests/stdio_integration.rs`) passes. Bug found during ticket 007 research (read_line → raw byte reads) fixed. Proceed to P1-03.

**Go/No-Go**: If stdio is reliable enough for production-grade tooling → proceed to P1-03 repo map and begin multi-agent prototype. If not, extend integration testing; may require architectural redesign before proceeding.

---

### Gate 1.2 — After Repo Map + Tree-Sitter (Month 3.5)

**Decision**: Does enhanced context awareness justify embedding complexity?

| Metric | Threshold | Measurement Method |
|--------|-----------|-------------------|
| Code understanding score | ≥15% improvement over baseline on query task | Internal benchmark suite (10+ questions) |
| Query response time | <2s median for file/project queries | User timing tests |
| False positive rate | <10% irrelevant results returned in top-5 | Human evaluation of sample queries |
| Maintenance burden | Grammar updates needed <1x/month | Calendar tracking |

**Go/No-Go**: If repo map demonstrably improves developer productivity without excessive maintenance → proceed to Phase 2 embeddings and P2-02 multi-agent pipeline. If minimal value added, consider simplifying or de-scoping the feature.

---

### Gate 2.1 — After Gate v2 Integration (Month 6)

**Decision**: Is enhanced quality gate sufficiently differentiated?

| Metric | Threshold | Measurement Method |
|--------|-----------|-------------------|
| False positive rate | <15% (per ROADMAP north-star) | Testing against known codebase patterns |
| Violation catch rate | ≥80% of structured/style violations caught | Comparison against standalone linter output |
| Performance impact | <100ms added to file save/check | Benchmarks integrated into editor workflow |
| Repeat error reduction | ≥30% decrease in recurring errors after auto-promotion | Historical data comparison in NegativeKnowledgeStore |

**Go/No-Go**: If Gate v2 delivers clear quality improvement with acceptable UX cost → continue to multi-agent pipeline enhancement. If FP rate stays high or performance impact blocks adoption, reconsider Gate architecture or scope.

---

### Gate 2.2 — After Multi-Agent Pipeline Production (Month 8-9)

**Decision**: Does multi-agent pipeline justify its 3× token cost and complexity?

| Metric | Threshold | Measurement Method |
|--------|-----------|-------------------|
| Unattended multi-file task success | ≥60% (ROADMAP north-star) | Internal test suite on complex tasks |
| Cost efficiency ratio | Improvement in success rate >3× token cost increase | Token usage metrics + success delta |
| Review agent bug detection | ≥20% additional bugs caught by Review agent vs Build alone | Blame analysis on bug fixes |
| Fix loop effectiveness | Delta-only retry resolves ≥75% of generated file issues | Failure/success counting in pipeline traces |

**Go/No-Go**: This is THE critical go/no-go gate for full Path B commitment. If multi-agent demonstrably delivers substantially higher success rates worth the complexity → proceed with Phase 2 remaining features (P2-03 through P2-07) and plan beta launch. If not, either pivot to simplified pipeline (Plan→Build→Gate only) or abandon multi-agent as premium feature only.

---

### Alternative Paths if Gates Are Failed

If Gate 2.2 fails (multi-agent doesn't deliver), options include:

1. **Path B' (Reduced)**: Ship multi-agent pipeline as premium feature gated behind subscription, while core product remains Path A (single-agent + strong Gate). Use the gate moat as primary differentiator.

2. **Path C (Platform)**: Pivot towards becoming a provider-agnostic platform where third-party agents can plug into the Gate + Pipeline infrastructure rather than building all agent capabilities in-house.

3. **Path Return**: Accept that multi-agent was too complex, consolidate onto single-agent loop with best-in-class Gate, focus on developer experience and documentation instead of chasing feature parity.

## Resource Allocation Triggers

| Condition | Action Triggered |
|-----------|------------------|
| Gate 1.1 AND Gate 1.2 BOTH pass with good margins | Increase team allocation to Phase 2; bring on additional developers |
| Gate 2.1 shows >50% improvement over baseline Gate | Prepare roadmap for Phase 3 (AI-driven code generation, predictive refactoring) |
| Any gate threshold NOT met within ±15% of estimate | Pause scope expansion; conduct retrospective; adjust next gate criteria downward |
| User acquisition stalls after alpha release despite passing Gate 0.1 | Re-evaluate market positioning; may need to reframe product messaging around differentiators |

## Final Investment Decision Point

**Before beginning Phase 2 resources**, once Gate 1.2 is passed, there should be a formal executive/stakeholder review presenting:
- Summary of all Phase 1 results against Gate criteria
- Updated financial forecast for completing Phases 2.x
- Competitive landscape update (any new entrants or moves by Claude/Codex)
- Recommended resource commitment level (full build, reduced build, pivot)

This is the definitive "commit to Path B or change course" meeting, occurring approximately Month 4 after alpha launch.

## Conclusion

The commitment to full Path B should never be a binary pre-alpha decision. Instead, it should be progressively validated through measured checkpoints where each major component must demonstrate competitive justification before enabling the next layer of complexity. The multi-agent pipeline (Gate 2.2) is the ultimate make-or-break decision—if it doesn't deliver substantially better outcomes than a well-tuned single-agent system with an excellent Gate, then the ambitious investment may not be justified. The hybrid strategy's strength lies in establishing early revenue/validation (Path A) while systematically de-risking the larger investment (Path B) through incremental gating.

**Status update**: Gates 0.1 and 1.1 are the only ones with concrete implementation status — 0.1 is pending alpha release, 1.1 is resolved (MCP stdio implemented + tested, bug fixed). Gates 1.2–2.2 remain future work tied to Path B Phase 1/Phase 2 execution. The framework is approved; execution depends on whether the team commits to building P1-03 (repo map) and P1-04 (real embeddings).

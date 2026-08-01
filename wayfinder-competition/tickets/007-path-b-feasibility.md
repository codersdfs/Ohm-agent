# Ticket: Is Full Path B Execution Technically Feasible?

## Question

Given the hybrid strategy decided in ticket 002 (ship Path A alpha first, keep Path B components gated behind experimental flags), what is the technical feasibility of executing the full Phase 1 + Phase 2 ROADMAP as described in Path B? Can all the complex components actually be built and integrated without fundamental architectural blockers?

## Current Path B Components (from ROADMAP)

| Component | Crate(s) Affected | Status | Notes |
|-----------|-------------------|--------|-------|
| Real MCP stdio client (`mcp`) | `mcp`, `omega-cli` | HTTP-only implementation exists | Need transport layer rewrite from HTTP to stdio JSON-RPC |
| Repo map + tree-sitter indexing | `memory`, `repomap` (new) | None implemented yet | Need to add Rust/TypeScript/Python grammars, caching layer |
| Real embeddings | `memory`, `entropy` | n-grams only | Need fastembed/ONNX integration, model weight management |
| Provider routing with health checks | `providers`, `omega-core` | Static enum only | Need dynamic routing, health probes, fallback logic |
| Working multi-agent pipeline | `omega-core`, `harness` | Experimental, behind flag | Plan→Build→Gate→Review→Fix needs production quality |
| Binary releases | `omega-cli`, CI/CD | Partial | Need GitHub Actions for Windows/macOS/Linux |
| Eval harness | `harness`, `evals` | Minimal skeleton | Need 20+ task suite, runner, baseline metrics |
| VS Code extension | `taste`, web UI stub | Minimal | Low effort but separate repo/toolchain |

## Critical Interdependencies

1. **Repo map depends on tree-sitter grammars** — need Rust/TS/Python grammar maintenance, potential conflicts with existing parsers
2. **Embeddings depend on ONNX runtime** — cross-platform binary compatibility issues, model cache management
3. **Multi-agent pipeline depends on Entropy GC** — drift scanning must work reliably for session state cleanup
4. **Provider routing depends on MCP stdio** — health check hooks need real tool communication channels
5. **Eval Gate depends on both multi-agent AND real embeddings** — quality metrics need reliable context recall

## Technical Risks

### High Risk Items
- **Tree-sitter ecosystem dependency**: Grammars may not maintain up-to-date syntax; could become a maintenance burden
- **ONNX Runtime on Windows**: Cross-platform binary size, distribution, and startup time concerns
- **State machine complexity**: Pipeline state machine (`state.rs`) needs to handle 5+ agents with error recovery — complexity grows combinatorially
- **MCP stdio transport rewrite**: Existing HTTP client is simple; stdio requires process lifecycle management, message framing, error recovery

### Medium Risk Items
- **Memory bloat**: Three-layer store + embeddings + repo map could exceed 500MB on large projects
- **Tool coordination**: 14 tools with varied output formats; building consistent wrappers for multi-agent pipeline is non-trivial
- **CI/CD matrix**: Building/test binaries for 3 OS × 2 arches = 6 build targets increases pipeline complexity

## Research Needed

- Verify tree-sitter grammars exist for all target languages (Rust, TypeScript, Python, JavaScript, Go, etc.) and assess maintenance commitments
- Benchmark ONNX Runtime download size, model warmup time, and memory footprint on Windows/macOS/Linux
- Evaluate state machine complexity by tracing the full Plan→Build→Gate→Review→Fix cycle edge cases
- Assess MCP stdio rewrite difficulty by comparing current HTTP implementation vs required stdio protocol

## Decision Points Before Committing to Path B

1. **Can we accept technical debt on tree-sitter grammars**? Outsource to community or commit to maintaining them internally?
2. **Is ONNX Runtime performance acceptable** for real-time embedding queries in the editor experience?
3. **Does the state machine complexity warrant extracting into a separate crate** or using an existing workflow engine?
4. **How many weeks would the MCP stdio rewrite realistically require** based on similar projects?

## Conclusion

Path B is technically feasible but requires substantial engineering effort across multiple domains. The high-risk items are addressable but represent significant unknowns that require research before full commitment. A phased approach—starting with P1-02 (MCP stdio) and P1-03 (repo map) while keeping multi-agent experimental—is recommended to validate each component incrementally before integrating the full pipeline.

## Research Outcome

**Status: RESEARCH COMPLETE → findings in `../research/007-findings.md`**

### Codebase corrections (PLAN_SUMMARY was overstated)
- `entropy` and `omega-core` now COMPILE (fixed by ticket 006). 203+ tests pass.
- CLI has 3 subcommands: `Chat`, `Exec` (headless loop), `ServeMcp` — not just 2.
- All 3 pipeline agents (Plan/Build/Review) exist but are gated behind `OMEGA_EXPERIMENTAL_PIPELINE=1`.
- State machine has 7 states (not yet including Gate/Fix): Idle, Planning, Building, Reviewing, Retrying, Completed, Failed.
- `mcp` crate client is HTTP-only; `mcp-server` already has both HTTP + stdio (but stdio uses naive newline framing, spec requires Content-Length).

### Four decision questions answered
1. **Tree-sitter grammar debt**: Acceptable with version pinning strategy. Grammars exist for all target languages. Maintenance burden is real but bounded.
2. **ONNX Runtime performance**: Acceptable for batch, tolerable for interactive. 2-5s warmup on first call (lazy-init mitigates). Binary size impact ~100-200MB. n-gram fallback available.
3. **State machine extraction**: NOT warranted. Current 130-line enum+match handles 7 states linearly. Adding Gate+Fix states → ~200 lines. No workflow engine needed.
4. **MCP stdio rewrite**: 3-5 weeks. Week 1: Content-Length framing parser. Week 2: subprocess lifecycle. Week 3: interop testing. Weeks 4-5: edge cases.

### Overall verdict
**Path B is feasible without fundamental architectural blockers.** The only real blocker (entropy compile) is resolved. All high-risk items have concrete mitigation paths.

**Recommended sequencing** (critical path first):
1. P1-02 MCP stdio (3-5w) → validates the biggest unknown
2. P1-03 repo map (2-3w) → on top of existing tree-sitter grammars
3. P1-04 real embeddings (4-6w) → swap n-gram for ONNX
4. P1-05 provider routing (2-3w) → add health checks to static router
5. P2-04 wire pipeline into CLI (2-3w) → ungate the 3 existing agents
6. P1-07 binary releases + P1-08 eval harness (parallel, 2-3w + 3-4w)

**Total: ~16-24 weeks.** VS Code extension and taste-1 ML model remain out of scope for Path B core.

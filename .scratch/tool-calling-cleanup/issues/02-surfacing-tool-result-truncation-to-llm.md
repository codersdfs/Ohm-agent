Type: task
Status: closed
Closed by: append truncation breadcrumb to result.output

## Resolution

`execute_tool_inner` (omega-core/commands/tools.rs) now consumes the
`BudgetCheck` from the pipeline and appends a one-line breadcrumb to
`result.output` when the result was truncated:

```
...[truncated, full output at <workspace-relative or absolute path>]
```

The LLM can use this marker to re-read the sidecar via the standard
`read` tool when it needs the full text. Path is rendered as
workspace-relative when the persisted file is inside the workspace
(useful as a hint that the file is in the model's own tree), absolute
otherwise.

## Changes

- tools.rs: `let (mut result, budget) = pipeline.execute(...)` and
  `if budget.truncated { result.output.push_str(...) }` after the call.
- tools.rs: added `render_truncation_path(path, workspace)` helper.
- tools.rs: two tests in the existing tests module covering both
  relative and absolute path rendering.

## Acceptance

- [x] `execute_tool_inner` consumes the `BudgetCheck` and produces
  the augmented `ToolResult`.
- [x] Path is rendered relative-to-workspace if inside it, absolute otherwise.
- [x] Tests in `commands/tools.rs` cover both rendering branches.
- [x] No regression: 230 passed, 0 failed (was 228 before this change).

ponytail: end-to-end test that drives a real truncation through
`web_fetch` / `grep` / `bash` and asserts the marker shows up in
the LLM-facing message is deferred — that needs a real LLM fixture
which is the wider chat-loop test gap ticket.

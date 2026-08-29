Type: task
Status: closed
Closed by: regression test + code review

## Resolution

After code review, the pipeline already populates `source_tool` on every
error path that knows the tool name:

- `pipeline.rs:89` — NotFound (uses `with_kind_and_source`)
- `pipeline.rs:229` — SchemaValidation (sets `source_tool: tool.name()...into()`)

The tool's own `call()` errors carry `source_tool` via the pipeline wrapping.
Hook errors carry it through the hook context. The ticket's premise that
"callers forget" was a misread of the current state.

## Changes

- `pipeline.rs`: added `test_not_found_error_carries_source_tool` — a
  regression guard that asserts any tool name passed to `execute` ends
  up in the error's `source_tool`. Catches a future refactor that drops
  the field by accident.

## Acceptance

- [x] `git grep "source_tool" src-tauri/crates/tool-harness/src/` shows
      pipeline always sets it for known error paths.
- [x] Test in `pipeline.rs` proves: `NotFound` error returned from
      pipeline has `source_tool = Some("missing_tool_name")`.
- [x] The two existing manual population sites (schema validator,
      `with_kind_and_source`) are kept; the manual population that was
      flagged in the ticket was already consolidated.

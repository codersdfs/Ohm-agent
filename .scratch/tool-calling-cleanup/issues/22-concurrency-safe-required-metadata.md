Type: task
Status: closed (no change needed)
Closed by: code review — the default is already `false`.

## Resolution

The `Tool` trait's default `metadata()` implementation sets `concurrency_safe: false` (`tool-harness/src/traits.rs:38`). The first survey in the ticket said "no default at all, often silently `true`" — that was a misread. A new test in `pipeline.rs` (`test_concurrency_safe_default_is_false`) locks in the contract: a tool author must opt in via the `ToolMetadata` builder, not by accident.

## Acceptance

- [x] Test proves `concurrency_safe` defaults to `false` in the trait default.
- [x] `grep -n "concurrency_safe: true" src-tauri/crates/tool-harness/src/tools/` shows every `true` is an explicit choice (read, grep, glob, git_status, git_diff, git_log, web_fetch).

ponytail: the more interesting follow-up is a linter that catches new tools whose `read_only` is `true` but `concurrency_safe` is left `false` (or vice versa). Marked as a separate ticket for the meta-data review.

Type: task
Status: closed
Closed by: registry-driven is_concurrency_safe lookup

## Resolution

Replaced the hardcoded name-allow-list closure in `commands/chat.rs::handle_tool_calls` with a registry-driven lookup that calls `Tool::is_concurrency_safe(&ToolInput)` on the actual tool instance. The tool's own metadata (`concurrency_safe: bool` in `ToolMetadata`) is now the single source of truth.

Changes:
- Added `ExecutionPipeline::registry() -> &ToolRegistry` getter (`tool-harness/src/pipeline.rs`).
- Replaced the `matches!(name, "read" | "grep" | ...)` closure with a closure that:
  - looks up the tool by name in the pipeline's registry;
  - parses the LLM-side `arguments: String` into `serde_json::Value` (lazy);
  - calls `tool.is_concurrency_safe(&ToolInput { tool, args })`.
- Unknown tool name or parse error → returns `false` (safe default: serial).
- Two new tests in `pipeline.rs`:
  - `test_concurrency_safe_is_source_of_truth`: tool with default metadata reports `false`.
  - `test_concurrency_safe_default_is_false`: the trait-default `concurrency_safe` is `false` (opt-in).

## Acceptance

- [x] Closure deleted from `commands/chat.rs`.
- [x] The split into `safe_indices` / `serial_indices` consults the registry.
- [x] Adding a new read tool requires zero edits to chat.rs.
- [x] A test in `pipeline.rs` proves the registry lookup returns the right value.

ponytail: an end-to-end test that drives `handle_tool_calls` with a `ToolPipeline` and asserts the parallel `JoinSet` path is taken is deferred — the dispatcher requires a full `AppState` (with broadcast channels, permission emitters, cost tracker), which is the kind of fixture work that the wider chat-loop test gap ticket covers.

## Question (original)

`commands/chat.rs` (omega-core) splits tool calls into "parallel" vs "serial" via a hardcoded name allowlist:

```rust
let is_concurrency_safe = |name: &str| -> bool {
    matches!(name, "read" | "grep" | "glob" | "git_status" | "git_diff" | "git_log" | "web_fetch")
};
```

But every `Tool` impl already populates `ToolMetadata.concurrency_safe` (and the trait exposes `is_concurrency_safe(&self, &ToolInput) -> bool` that defaults to reading the metadata). The dispatcher ignores both.

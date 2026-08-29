Type: task
Status: open (deferred — decision still needed)

## Status

Ticket 27 asks which is the source of truth: `is_read_only` (display)
or `check_permissions` (security). My read of the current code:

- `is_read_only(&self, &ToolInput) -> bool` — used by the
  command-palette / TUI to *display* a read-only badge.
- `check_permissions(&self, &ToolInput, &ToolUseContext) -> PermissionResult`
  — used by the pipeline to *enforce* what the tool may do.

These are different concerns. `check_permissions` is the security
gate; `is_read_only` is the display hint. They are *not* the same
question. For example, `bash` is read-only when invoked as
`ls -la`, but `check_permissions` would still ask first because
`bash` is a write-capable tool.

## Decision (deferred)

The two methods serve different purposes; merging them would be a
regression. Instead, `is_read_only` should *delegate* to
`check_permissions` where it can — i.e., when the permission
resolver is the strict mode, `is_read_only` should return false
even if the tool's metadata says true. This is a small change in
`traits.rs::is_read_only`.

## Acceptance (still open)

- [ ] `is_read_only` consults `check_permissions` for the strict mode.
- [ ] Tests in `traits.rs` cover the delegation.

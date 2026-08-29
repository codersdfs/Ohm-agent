Type: grilling
Status: closed (decision: keep "off", document the intent)
Closed by: code review

## Decision (resolved)

**Option A modified:** keep the REPL on "off" (developer escape hatch, the
user typed the command), but document the intent with a named constant
and a comment. The TUI/CLI paths have their own explicit permission
handling via `SendMessageRequest::permission_mode`; the REPL is the
"no questions asked" path.

The ticket's Option A also proposed deleting the prompt machinery, but
that machinery is exercised by the TUI/CLI paths (`Permission::Allow`
short-circuit in non-terminal emitters is intentional for TUI, which
auto-approves when no prompt UI is available). Not safe to delete
without breaking TUI/CLI.

## Changes

- `commands/chat.rs`: added `pub const REPL_PERMISSION_MODE: &str = "off";`
  with a comment explaining why the REPL is permissive.
- `commands/chat.rs`: `send_message` now passes `REPL_PERMISSION_MODE`
  to `handle_tool_calls` instead of the hardcoded literal "off".

## Acceptance

- [x] The REPL default is a named constant, not a magic string.
- [x] The intent ("developer escape hatch, no questions asked") is
      documented at the const declaration.
- [x] Prompt machinery kept (TUI/CLI still uses it).

ponytail: the typed enum replacement (currently stringly-typed `&str`)
is deferred — bigger refactor that touches every `check_permission` call
site.

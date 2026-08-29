Type: task
Status: closed (resolved by ticket 26)
Closed by: REPL_PERMISSION_MODE const

## Resolution

Ticket 26 chose "keep off, document the intent" and that decision is
implemented. `send_message` now uses `REPL_PERMISSION_MODE` instead
of the magic string "off". The TUI/CLI default is unchanged (they
read from `SendMessageRequest::permission_mode`).

## Acceptance

- [x] The REPL default is a named const, not a stringly-typed literal.
- [x] The TUI/CLI default path is unchanged.
- [x] Workspace tests still green.

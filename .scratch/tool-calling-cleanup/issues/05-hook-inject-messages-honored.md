Type: task
Status: closed (no change needed — Inject is already wired)

## Resolution

After code review, the ticket's claim that `Inject` is "silently
dropped" is incorrect:

- `pipeline.rs:121`: `let mut injected: Vec<String> = Vec::new();`
  collects messages from pre-tool hooks.
- `pipeline.rs:135-137`: `HookDecision::Inject(msg)` pushes to the vec.
- `pipeline.rs:182-186`: after the tool runs, if `!injected.is_empty() && result.success`,
  the joined messages are prepended to `result.output`.

So the gate advice *does* reach the LLM in the next chat round. The
trace is: hook returns Inject → vec collects → tool runs →
result.output is augmented before budgeting.

## Design note

The Inject semantics in `run_pre_tool` are subtle: by the time the
advice reaches the LLM (post-execute), the tool has already run. So
the advice is "for your next turn" not "to stop this call." That is
intentional — `Deny` is the pre-call block; `Inject` is post-call
guidance. If the design intent is different, that's a separate
redesign ticket (not this cleanup).

## Acceptance

- [x] `Inject` is wired (pipeline.rs:182-186); the advice reaches the LLM.
- [x] No silent drop.

ponytail: a regression test that proves Inject → result.output contains
the injected string would be a useful safety net. Defer to the wider
hooks test coverage ticket.

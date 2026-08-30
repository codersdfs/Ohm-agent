# Session handoff

How to start the next session. Three commands and a checklist.

## What to do first (3 commands)

1. `cd C:/omega-agent`
2. Open `cat .scratch/project-roadmap/tasks/tomorrow.md` (or
   `this-week.md`). That is the day's plan.
3. Run `cargo test --workspace` to confirm the workspace is still green
   from the previous session.

## Checklist at the start of every session

- [ ] Is `main` green? (`git log --oneline -5` + `cargo test --workspace`)
- [ ] Is the task board up to date? (Re-read `task-board.md` to make sure
      nothing was added since last session.)
- [ ] Is the current state snapshot still accurate? (Re-read
      `current-state.md`. If the numbers moved, update it.)
- [ ] Is there a postmortem for the last release? (If you cut a release
      since the last session, there should be one in `.scratch/`. If not,
      write it before starting new work.)

## Checklist at the end of every session

- [ ] All work committed and pushed. (Or, if not, a clear note in the
      commit message about what is WIP.)
- [ ] Any new ticket filed in the task board with a P-bucket and an
      estimate.
- [ ] `cargo test --workspace` still green. (If you broke a test, fix it
      or revert. Don't leave a red tree at end of session.)
- [ ] A one-paragraph summary in the day's `tasks/<date>.md` (create it
      if it doesn't exist).
- [ ] Any postmortem written for the day's surprises. ("Surprise" =
      anything that took 2x longer than expected, anything that changed
      the design, any new finding about the codebase.)

## What to do if you have 2 hours

Pick the highest P-bucket item that fits. If nothing P0 fits, do a
P1/P2 ticket. If nothing in the board is "ready to work" (needs research,
blocked, unclear scope), do research. Research is fine. A 2-hour session
that produces a one-paragraph research note is a successful session.

## What to do if you have 30 minutes

Two options:

1. **Triage the task board.** Read all P0/P1 tickets. Update estimates.
   Move things around if priorities have shifted. The board is the input
   to every other session; a 30-minute triage is more valuable than a
   4-hour partially-done feature.
2. **Write a postmortem for a recent surprise.** If you cut a release,
   a tag, a workflow change, or anything that involved more than 3 fixes,
   write the postmortem. Even 30 minutes is enough for the top 3 fixes.

## What to do if you have 15 minutes

Update `current-state.md` with the latest numbers. This is the
single-source-of-truth for "where are we", and stale numbers are worse
than no numbers.

## What NOT to do at the start of a session

- Don't re-read the entire roadmap. Tomorrow's plan and the current
  state are enough.
- Don't start a new feature without a ticket. Add the ticket first,
  then start the work. The discipline of "ticket before code" is what
  keeps the board honest.
- Don't pick a P2 ticket when a P0 is blocked. If the P0 is blocked,
  unblock it (it's a 30-minute task in 9 cases out of 10). If it can't
  be unblocked, the project is in a different state than the board
  thinks, and you should re-plan before continuing.

## Emergency stop

If you find yourself 2 hours into a task with no clear path to a green
test, **stop**. Revert the work. Write a one-paragraph note in the
task board about what's blocking. The next session will look at it
with fresh eyes. A clean revert is a successful session; a half-done
feature is a 2-day cost next time.

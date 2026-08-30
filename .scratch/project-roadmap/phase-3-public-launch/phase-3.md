# Phase 3 — Public Launch

## Goal

Get the product in front of 100 real users. The product is shipping at
v1.0.0; the work in Phase 3 is distribution, feedback, and the first
v1.x iterations based on what users actually do.

## Entry condition

v1.0.0 is published. The Phase 1 benchmark is on the README. A new user
can install and use the product without a human in the loop.

## Exit condition

- At least 100 users have run `omega` at least 3 times in a 30-day
  window.
- At least 10 users have run the Gate (`omega gate ./src/` or
  equivalent) and reported the result on a GitHub issue or Discussion.
- At least 1 user has filed a feature request that gets implemented in
  v1.1 (proves the user-feedback loop works).
- At least 1 user has filed a bug that gets fixed in v1.0.x (proves the
  support loop works).

## Work in this phase

### 3A. Distribution

- **GitHub presence:** the README is the marketing site. Improve it
  until a stranger can understand the niche in 60 seconds.
- **Announcement channels:** pick 2-3 places to post the v1.0.0 release.
  Candidates: Hacker News (Show HN), r/LocalLLaMA, the Rust subreddit,
  X/Twitter, a relevant Discord. One well-targeted post is better than
  five scattershot ones.
- **Installation polish:** the binary install should be one command
  (`cargo install omega` or a curl-able shell script). The current
  `cargo install` from a git URL is fine for v0.x but not for v1.0.

Estimated 1 week. Most of this is the announcement, not the install.

### 3B. Feedback triage

- Every GitHub issue and Discussion gets a response within 48 hours.
  Even a "thanks, I'll look at this" is a response. Users stop filing
  issues when they feel ignored; the goal is to feel heard.
- Categorize feedback: bug, feature request, design discussion, "this
  doesn't work for my use case". Bugs go to the P0 board. Features go to
  P2. Design discussions get a thoughtful reply, then a ticket or not
  based on the discussion.
- The "doesn't work for my use case" category is the most valuable.
  These tell you where the niche assumption is wrong. Take them
  seriously, even if the answer is "this is not the tool for you."

Estimated ongoing; 2-3 hours per week through Phase 3.

### 3C. v1.0.x iterations

- The first 4-6 weeks of Phase 3 are mostly bug fixes and small
  improvements. Each is a small commit with a regression test.
- New features are deferred to v1.1 unless they fix a real user pain.
- The eval suite should grow. Every new user-reported failure mode is a
  candidate for an eval task.

### 3D. v1.1 cut

- Roughly 6-8 weeks after v1.0.0.
- Should include: a handful of user-requested features, an expanded
  eval set, and updated competitive benchmark numbers.
- Should NOT include: an architectural rewrite, a new niche, a new
  pricing model. v1.1 is the same product, slightly better.

## What is NOT in Phase 3

- **No pricing or monetization until the user count is real.** No
  one wants to pay for a tool with 12 users. Get the count first, then
  ask if anyone would pay.
- **No marketing site / landing page / blog.** A README is enough at
  this scale. The v2.0 work (post-Phase 3) is when the marketing site
  becomes worth building.
- **No "we are the AI agent platform" vision.** v1.x is a tool. The
  platform question is a v3 question.

## Risks

| Risk | Mitigation |
|---|---|
| No users show up (the niche is wrong) | The competitive benchmark from Phase 1 is the data. If the data shows the Gate works, the niche exists; we just have to find it. Try different announcement channels. Talk to specific teams who would benefit. |
| Users show up but find bugs faster than we can fix them | Triage ruthlessly. P0 = data loss, security, broken install. P1 = agent loop crashes. P2 = everything else. Don't apologize for closing P2 as "won't fix in v1.x, will reconsider for v2." |
| Users request features that pull the project toward Claude Code (multi-agent, web UI, IDE extension) | That's a market signal, not a green light. Each request goes in the task board. If 5 users request the same thing, it moves to P1. Until then, it stays at P2. |
| The user-feedback loop turns into a support burden (2 engineers spending all their time triaging) | Set a weekly cap: 5 user-facing replies, 2 issue closes. Beyond that, the response is "we will look at this in the next release." Users can wait. |

## Definition of done for the phase

When v1.1 is published and the user count is real (>= 100 active monthly
users, definition above), Phase 3 is done. The next phase is v2 planning,
which is a different document.

## What comes after Phase 3

The v2 plan is not in this folder. It depends on what Phase 3 teaches us.
Possible v2 directions, none committed to:

- **Org/team features** (shared rules DB, shared negative-knowledge). If
  teams adopt Omega, the next obvious need is team-level state.
- **Cloud-hosted** (the binary is great for tinkerers; teams want
  managed).
- **Gate-only mode** (sell the Gate as a CI step, not an agent). The
  competitive benchmark might show this is the stronger product.
- **Vertical integration** (one specific language ecosystem, deeply
  integrated). If the niche turns out to be Rust shops, this is a real
  option.

These are guesses. Phase 3 is what turns them into data.

Type: task
Status: closed
Closed by: dropped the unused mcp dep

## Resolution

`tool-harness/Cargo.toml` had `mcp = { path = "../mcp" }` but a grep
across the tool-harness source for any `mcp` symbol returned zero
matches. The dep was a leftover from a previous design that pulled in
the Skill type — that re-export was removed at some point and the dep
stayed.

Removing it shaves one line off the dep graph and removes a phantom
`reqwest` pull-in (since `mcp` depends on `reqwest` for the JSON-RPC
HTTP transport; `tool-harness` already has its own `reqwest` for
`web_fetch.rs`). One less duplicate `reqwest` client in the binary.

## Acceptance

- [x] `cargo tree -p tool-harness | grep mcp` returns nothing.
- [x] `cargo check -p tool-harness` succeeds; workspace tests green.
- [x] No behavior change.

ponytail: the wider arrow (mcp-client depending on tool-harness types
or extracting a tool-harness-types microcrate) was deferred — the
`mcp` crate is small and the boundary is stable for now. If/when
`mcp-server` also wants to consume tool-harness types directly,
re-evaluate.

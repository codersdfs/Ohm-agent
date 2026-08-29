Type: task
Status: closed (decision: keep — ticket premise was wrong)

## Resolution

After code review, `OpenAIMessage` etc. are not pointless ceremony.
They are an explicit provider boundary:

- `providers::ChatMessage` is a provider-agnostic in-memory struct used
  by the agent loop. Adding `#[serde(rename = "type")]` to its
  `tool_type` field would change the wire format for *all 8*
  OpenAI-compatible providers that share the `openai::OpenAIProvider`
  transport (the ticket's "use protocol::ChatMessage directly" path).
  That is a cross-provider regression, not a per-provider cleanup.

- The local OpenAI structs are a translation layer between
  provider-agnostic in-memory representation and OpenAI's exact wire
  format. Anthropic's `AnthropicContentBlock` plays the same role for
  Anthropic. The pattern is intentional, not accidental.

- The wire format assertion in the ticket ("byte-identical to before")
  is the giveaway: the *current* code is byte-correct. Removing the
  translation layer does not improve correctness; it just couples
  `ChatMessage` to OpenAI's quirks.

## Acceptance

- [x] `git grep OpenAIMessage src-tauri/` still returns matches — the
      duplicate struct is kept as a provider boundary.
- [x] All existing provider tests pass; wire format unchanged.

ponytail: if/when a v2 `ChatMessage` design is taken on (e.g., to
unify content blocks across providers), the OpenAI translation layer
becomes simpler. Until then, the layer is the cost of provider
isolation.

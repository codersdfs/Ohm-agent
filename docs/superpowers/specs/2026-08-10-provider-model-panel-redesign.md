# Provider / Model Panel Redesign

**Date:** 2026-08-10
**Status:** Draft — awaiting review
**Scope:** `src-tauri/crates/omega-core/src/tui/provider_panel/` + a small `providers` API surface. No changes to chat loop, config loading, or provider transports.

## Problem

The current provider/model panel is a **full-screen 3-step wizard** that treats every provider as needing the same (mostly irrelevant) fields. Three concrete pains:

1. **The "Advanced" step (Base URL + API key + tokens) feels required.** Hosted providers (OpenAI, Anthropic, …) use built-in base URLs and an env-sourced API key; the user is forced past a screen they don't need just to finish configuring a model.
2. **No progress/context.** There is no visible sense of which step you're on or what your current selection is while you move through the flow; the only summary is a one-liner at the top.
3. **Lists/clunky navigation.** The provider list is a plain column you must arrow through (number-jump reaches only 10 of 15 providers); the model list shows no provider context while you browse.

## Goals

- The common hosted-provider flow is exactly **two screens**: Provider → Model → Apply. "Advanced" (url/tokens/temp) is a first-class, self-labeled screen entered **only** for providers that genuinely need it, and reachable manually otherwise.
- The step you're on and the current selection are always visible.
- Both lists are type-to-filter and fit their content without awkward paging.

## Non-goals / out of scope

- No change to the chat loop, `AppState.provider_config` persistence, config file/env loading, or any `LlmProvider` transport.
- No change to provider discovery/fetch (`models::fetch_models`), only to how the panel surfaces it.
- No non-modal or split-pane layout. The panel remains a full-screen modal (the user explicitly chose to keep the wizard shape).

## Flow & steps

| Step | Contents | Terminal action |
|------|----------|-----------------|
| 1 **Providers** | Searchable grid of all 15 providers | Enter → Model |
| 2 **Model** | API-key bar (hosted only) + model list | Enter → Apply (hosted) **or** → Advanced (Custom/Local) |
| 3 **Advanced** | Base URL, Max Tokens, Temperature | Enter → Apply |

- **Esc** walks back one step (Model → Providers → Close); unchanged.
- **Ctrl+Enter** applies from any step; unchanged.
- **Ctrl+A** opens Advanced manually from the Model step (any provider) for users who want to override URL/tokens/temperature.

### When Advanced appears

- **Auto-entered** after Model only when `ProviderKind::needs_advanced() == true`: **`{Custom, Local}`**.
- **Local**: no API-key bar on Model (Ollama needs none); Advanced offers the base URL (default `http://127.0.0.1:11434`) + tokens.
- **Custom**: API-key bar on Model; Advanced offers Base URL (default `https://your-endpoint/v1`) + tokens.

## Header (every step)

Replaces the current one-line summary:

- **Step indicator:** progress dots + `Step N/M` (e.g. `● ● ○ · Step 2/3`). `M` is 2 for hosted providers, 3 when Advanced is in play.
- **Live selection chip:** `Current: openai · gpt-4o`, updated as the user moves. Reflects the selected provider/model immediately, so "what am I changing from → to" is always visible.

## Step 1 — Providers (searchable grid)

- **Type-to-filter** search field at the top, reusing the model step's existing filter machinery (`FilteredList`, `rank_model`-style ranking scoped to provider names).
- **2–3 column compact grid** so all 15 fit on a normal terminal without scrolling.
- **Number-jump reaches all 15**: two-digit support (type tens then units; e.g. `1` then `5` → provider #15). `0` = #10, unchanged.
- Footer: `type filter · ↑↓/jk move · Enter model · Esc cancel`.

## Step 2 — Model

- **Credential bar** at the top, visible only when `needs_api_key()` — i.e. every provider except `Local`: `API key: ▸ ___`, prefilled from `config.api_key` / env.
- When the key is non-empty → **fetch** models → list below. Preserve existing spinner and error states.
- **List header now carries provider context:** `Models · openai · 12 match/40`.
- **Current model pinned** with `★ current` (unchanged semantics; kept visually stable).
- Enter → Apply (hosted) or → Advanced (Custom/Local); `Ctrl+A` → Advanced (manual); Esc → Providers.

## Step 3 — Advanced

- **Base URL** field (prefilled `default_base_url()`), **Max Tokens**, **Temperature**.
- Footer: `Enter Apply · Esc Model`.

## New / changed API & state

- `providers::ProviderKind::needs_advanced(&self) -> bool` — `true` for `{Custom, Local}`.
- `providers::ProviderKind::needs_api_key(&self) -> bool` — `true` for all except `Local`.
- `ProviderPanelState` gains:
  - provider search buffer + cursor and a `FilteredList<String>` for the provider grid;
  - a two-digit pending-jump buffer for provider number selection;
  - `API key` uses the existing `key_buffer`/`key_cursor`, now rendered on the Model bar (moved out of the old "Connection" section).
- `go_next` for `WizardStep::Model` branches: `needs_advanced()` ? → `Advanced` : `Apply`.
- Header render: progress dots + step count + live selection chip.

## Error handling

- Provider/model fetch failures continue to surface in their existing list-header error states; no new error surfaces.
- Keys: empty/short keys do not auto-fetch; fetch triggers on a non-empty key (typed or prefilled). Malformed base URL on Custom/Local is rejected at Apply time by the existing `create_provider` error path (no new validation).

## Testing

- Extend `omega-core/src/tui/provider_panel/mod.rs` tests (existing inline `#[cfg(test)] mod tests`):
  - `needs_advanced()` true for Custom/Local, false for hosted.
  - `needs_api_key()` false only for Local.
  - `go_next` from Model → Apply for a hosted provider; → Advanced for Custom/Local.
  - Provider search narrows the grid; two-digit number-jump selects provider #15.
  - Model header includes provider name.
- No project-wide test suite command; run `cargo test -p omega-core` for the touched crate.

## Migration

- The API-key field leaves the "Connection" section; "Generation & Apply" collapses into Advanced's fields. No persisted state changes (fields already live on `ProviderPanelState` / `ProviderConfig`).

# Ticket: Fix Entropy Compile Error or Quarantine It?

## Question

The entropy crate (src-tauri/crates/entropy/src/scanner.rs) calls
Language::detect(&paths) — a method that does not exist on the Language
enum (only Language::from_str exists in harness/src/language.rs). This
blocks compilation of omega-core (which depends on entropy) and therefore
the entire Omega CLI binary.

This is a blocking task — the competition assessment can not proceed until the
build is green or entropy is removed from the dependency chain.

### Options
1. **Fix it**: implement Language::detect(paths) in harness/src/language.rs
   (detect language by scanning for Cargo.toml, package.json, requirements.txt,
   etc.) and make entropy compile. Effort: ~30 min.
2. **Quarantine it**: remove entropy from omega-core Cargo.toml dependencies
   and gate the entropy scan behind a separate binary/crate. The drift scanner
   and GC become a standalone tool, not blocking the CLI.
3. **Delete it**: remove the entropy crate entirely (ROADMAP P2-06 says real
   MVP — but it is currently stubs anyway).

### Decision needed
- Which option unblocks the build with the least risk and most future value?
- Does entropy have any real code (beyond stubs calling cargo fmt /
  cargo clippy --fix)? Review gc.rs — it is a real but minimal implementation.
- Is entropy on the critical path for the competition moat, or is it pure polish
  that can ship later?
- If we quarantine/delete, what gets removed from the README and ROADMAP?

### Research needed
- Read all three entropy source files (lib.rs, scanner.rs, gc.rs) — done
  in PLAN_SUMMARY.md; confirm scope.
- Check harness/src/language.rs to see the exact Language enum surface
  available for a detect implementation.
- Check omega-core/src/lib.rs — what actually imports/uses entropy?
  (Only commands/entropy_cmd.rs if it exists.)

## Type: task

## Status: open

## Assigned to: (unclaimed)

## Resolution

### Decision

**Option 1: Fix it.** This was the correct choice, and it was executed.

### What was done

1. **Fixed `entropy/scanner.rs`** — the `detect_language()` function called `Language::detect(&paths)`, a method that did not exist on the `Language` enum (only `Language::from_str` existed).

2. **Implemented `Language::detect(paths: &[String])` in `harness/src/language.rs`** — a new method that scans the provided directory entry paths for well-known manifest files:
   - `Cargo.toml` → `Language::Rust`
   - `package.json` → `Language::TypeScript`
   - `pyproject.toml` / `requirements.txt` / `setup.py` / `setup.cfg` → `Language::Python`
   - `go.mod` / `go.sum` → `Language::Go`
   - `.csproj` / `.sln` → `Language::CSharp`
   - `pom.xml` / `build.gradle` / `.java` → `Language::Java`
   - Falls back to `Language::Other("unknown")`

3. **Added `Language::label()`** in `harness/src/language.rs` — the `plan.rs` pipeline code calls `.label()` on `detected_language` (which didn't exist, causing a second compile error once entropy was fixed). Returns a human-readable label for prompt injection (e.g. "Rust", "TypeScript (React)").

4. **Regenerated `Cargo.lock`** — the existing lockfile was corrupt (truncated checksum on `anstyle v1.0.14`). Deleted and regenerated with `cargo generate-lockfile`.

### Scope and risk

- **Minimal, surgical changes**: 2 new small methods on `Language`, no behavior changes to existing code paths.
- **entropy/scanner.rs was left untouched** in its logic — it already had the right intent (detect language, scan domains). The fix was purely providing the missing method it called.
- **No downstream regressions**: `cargo check -p omega-core -p omega` succeeds. `cargo test -p entropy -p harness` → 63 tests pass. `cargo test -p omega-core` → 135 tests pass (3 ignored integration tests requiring live services).
- **Entropy GC is preserved** as a real (now compiling) crate: `DriftScanner` runs the Gate across all source files per domain, `GarbageCollector` runs `cargo fmt` + `cargo clippy --fix`. The drift scanner is a legitimate future part of the moat (Gate-as-repo-whole-codebase-scan), so deleting it would have lost a valuable asset.
- **Future limitation**: GC currently only supports Rust (rustfmt/clippy only). Multi-language GC (eslint --fix, etc.) is a separate enhancement, flagged as out-of-scope for this unblock.

### Why fix vs quarantine vs delete

- **Fix** (chosen): ~1 hour, 2 small methods, unblocks the entire workspace build, preserves Entropy GC as a shipping-competent drift scanner. Zero risk of losing functionality.
- **Quarantine** would have required refactoring `omega-core`'s dependency graph to remove entropy from the default build path and gate it behind a feature flag — more churn, same end result (entropy compiles but unused).
- **Delete** would have permanently removed the drift-scanner concept (which aligns with the Gate moat — scanning an entire repo's drift using the same deterministic engine). Ticket 001's moat analysis notes entropy *could* become part of the moat once unblocked. Deleting it would foreclose that path.

### Verification

```
$ cargo check -p omega-core -p omega
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.60s

$ cargo test -p entropy -p harness
    test result: ok. 63 passed; 0 failed; 0 ignored

$ cargo test -p omega-core
    test result: ok. 135 passed; 0 failed; 3 ignored
```

## Type: task

## Status: closed

## Assigned to: omega-wayfinder

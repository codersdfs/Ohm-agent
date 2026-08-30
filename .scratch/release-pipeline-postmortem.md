# v0.1.0 Release Pipeline Postmortem

**Tag:** v0.1.0
**Target commit:** 54c4730 (the merge of PR #2)
**Result:** shipped after 10 pipeline fixes; 4 platform binaries, all
cosign-signed, with consolidated SHA256SUMS and SPDX-JSON SBOMs.
**URL:** https://github.com/codersdfs/Ohm-agent/releases/tag/v0.1.0

## Summary

The release pipeline (`.github/workflows/release.yml`) was authored in
`18cd4f6` but never actually run end-to-end against a real `v*.*.*`
tag. Cutting v0.1.0 exposed ten real bugs. Each is a different class
of failure, so the fixes are not a single coherent patch — they are
ten independent lessons.

The 11th failure was the discovery that the pipeline had never been
tested at all; the test was the release itself. Future tags should
be preceded by a `workflow_dispatch` dry-run on a non-tagged commit so
these bugs surface in the cheap path, not the irreversible path.

## The ten failures

### 1. Wrong binary path in stage-artifact step (5ed7643)

**Symptom:** Build succeeds. Stage step fails with `cp: ... No such
file or directory` on all four platforms.

**Root cause:** The build step ran `cargo build` with
`working-directory: src-tauri/crates/omega-cli`, but the workspace
`Cargo.toml` lives at the repo root with members under
`src-tauri/crates/*`. Cargo uses the workspace root for the target/
directory regardless of where the command was invoked from. The stage
step assumed the binary would be at
`src-tauri/crates/omega-cli/target/<triple>/release/omega`; it was
actually at `target/<triple>/release/omega` at the repo root.

**Lesson:** When a workflow runs `cargo build` from a member crate,
trust cargo to use the workspace target. Do not mix a per-crate
working-directory with a workspace-relative output path. **Always
resolve the artifact path from the workspace root, not the build
working directory.**

### 2. SBOM step had the same wrong path (2995947)

**Symptom:** After fixing #1, builds pass, but the syft SBOM step
fails with the same `No such file` error.

Root cause identical to #1; the same fix applies to every step that
references the build output. The fix duplicated the relative-path
convention; the deeper fix would be to compute the artifact path once
and reuse it via a workflow-level env var. (Not done; the duplication
is small and the gain is marginal.)

### 3. Syft v1.x flag syntax changed (2995947)

**Symptom:** After fixing the path, syft exits with
`unknown flag: --output-file`.

**Root cause:** The workflow was written against syft v0.x where
`--output <format>` and `--output-file <path>` were two flags. In
v1.x (the version anchored by `anchore/sbom-action/download-syft@v0`)
they collapsed to `-o <format>=<file>`. The action does not pin to
a specific syft version, so the action's "current" version silently
broke the flag syntax.

**Lesson:** Pin third-party CLI tool versions in workflows, even when
using installer actions. Syft v0.x and v1.x have different flag
shapes; the workflow should declare which one it expects. Add
`syft --version` to a verify step before relying on flags.

### 4. `sha256sum` not in default Windows PATH (60ae579)

**Symptom:** Build succeeds on Windows; stage step succeeds; SHA-256
step fails with `sha256sum: command not found`.

**Root cause:** `sha256sum` is GNU coreutils. The windows-latest
GitHub runner has a minimal Git Bash that does not include coreutils
in the default PATH. Even when Git Bash is the runner default,
`sha256sum` is not always present.

**Lesson:** GitHub Actions `runs-on: windows-latest` defaults to
PowerShell, with Git Bash as a fallback. **Any shell script that
uses GNU coreutils (`sha256sum`, `md5sum`, `shred`, etc.) will fail
on Windows** unless (a) the script uses PowerShell's Get-FileHash,
(b) the workflow installs coreutils explicitly, or (c) the script uses
a tool that is on every runner. Python is on every runner and ships
with `hashlib` — use it for hashing.

### 5. `zip` not in default Windows PATH (ef74e52)

**Symptom:** After fixing #4, the Windows build fails with
`zip: command not found`.

**Root cause:** Info-ZIP `zip` is not in the windows-latest runner.
`bsdtar` (which can create .tar.gz) is — Windows 10+ ships it
natively.

**Lesson:** Same shape as #4: a unix tool was assumed to be on
Windows. The fix was to switch the Windows matrix entry from
`archive: zip` to `archive: tar.gz`. If you genuinely need .zip on
Windows, the `KJK::CERN::zip-action` is one option; switching the
archive format is simpler. (The release notes should call out that
Windows binaries ship as .tar.gz, not .zip, since this differs from
the convention Claude Code and other tools set.)

### 6. PowerShell does not accept `\` line continuations (995bf35, 97ab5f9)

**Symptom:** On Windows, the cosign sign step and the syft SBOM step
both fail with `ParserError` at the first `\` line continuation.

**Root cause:** The steps used bash-style multi-line scripts (joined
with `\`). On Windows, the default step shell is PowerShell, which
does not accept `\` as a line continuation. (PowerShell uses
backtick `` ` `` for line continuation, or just a single line.)

**Lesson:** Multi-line `run:` blocks in a workflow that may run on
Windows MUST declare `shell: bash`. Otherwise the matrix entry is
hostage to the runner default shell. This is easy to miss because the
script parses fine on linux/macos where bash is the default. **The
right reflex is: any multi-line bash script in a `run:` block gets
`shell: bash` regardless of the runner, so the workflow is portable.**

### 7. cliff.toml used an invalid TOML pattern (f4f7267)

**Symptom:** `git-cliff --verbose --tag v0.1.0 --latest` fails with
`ConfigError(TOML parse error at line 33, column 40)`.

**Root cause:** The config had

```toml
  [commit_parsers]
  message = "..."
  body    = "..."
  [commit_parsers.group]   # sub-section of a *single* table
  ...
```

TOML does not allow fields directly on a single section AND a
sub-section of the same name. The intent was three separate parser
rules; the syntax collapsed them into one mixed section. Fix: use
`[[commit_parsers]]` (array-of-tables) for the three rules.

**Lesson:** `cliff.toml` configs that are copy-pasted from older
versions of git-cliff often mix `[table]` and `[table.subtable]` in
ways that worked in 1.x but are rejected in 2.x. The cliff 2.x error
messages point to a line and column; trust them and look for
duplicate `[[ ]]` vs `[ ]` patterns.

### 8. TOML basic strings reject `\s`, `\(`, etc. (4f27fe7)

**Symptom:** After fixing #7, cliff still fails with
`ConfigError(TOML parse error at line 37, column 40)`,
`missing escaped value, expected b, f, n, r, \, ", u, U`.

**Root cause:** TOML basic strings only allow a small set of escape
sequences: `\b`, `\f`, `\n`, `\r`, `\t`, `\"`, `\\`, `\uXXXX`,
`\UXXXXXXXX`. Any other backslash escape (like `\s`, `\(`) is a
TOML parse error. The regex string `"\s"` in git-cliff needs to be
written as `"\\s"` in TOML — two backslashes, which TOML decodes to
one backslash followed by s, which is what git-cliff sees.

**Lesson:** When porting regexes (which are full of `\s`, `\d`,
`\w`, `\(` etc.) into TOML, every backslash that is NOT a valid
TOML escape must be doubled. A regex like `\s+` becomes `"\\s+"`
in TOML. This is the same rule that applies to regexes in JSON
config, but TOML's restricted escape set is stricter than JSON's.

### 9. `cosign` was not installed in the publish job (4b786e4)

**Symptom:** Per-platform builds sign their tarball fine. Publish job
fails to sign SHA256SUMS with `cosign: command not found`.

**Root cause:** The per-platform build jobs each installed cosign
(`sigstore/cosign-installer@v3`) before their sign step. The publish
job added a sign step but did not add a corresponding install step.

**Lesson:** Every job that needs a CLI tool must install it. There
is no global install state across jobs. When duplicating a step
across jobs, also duplicate the install/setup steps. (Some teams
use a composite action to bundle install+run; that would have caught
this.)

### 10. `sign SHA256SUMS` ran from the wrong directory (aaab474)

**Symptom:** cosign runs but errors with `open SHA256SUMS: no such
file or directory`.

**Root cause:** The `compute SHA256SUMS` step had
`working-directory: artifacts` so the file landed at
`artifacts/SHA256SUMS`. The sign step had no working directory, so
cosign ran from the repo root and could not find the file.

**Lesson:** When a step's input comes from a previous step that set
a working directory, the consuming step must set the same working
directory, OR the producing step must `cp` the file to the repo
root. The default of "the workflow's default working directory" is
silent and easy to miss when reading the workflow in isolation.

## What I would do differently next time

1. **Run a dry-run on every workflow change.** GitHub Actions
   supports `workflow_dispatch` with manual `dry_run: true` inputs.
   The release workflow has a `dry_run` input, but it was never
   exercised before tagging. Every workflow edit should trigger a
   dry-run before the next real tag.

2. **Test on a non-tag commit first.** The `v0.1.0-rc1` pattern:
   push a non-tag commit, manually run the release workflow with a
   fake tag, fix what breaks, then cut the real tag. This is
   reversible; tagging is not.

3. **Pin tool versions in the workflow file.** Syft 0.x vs 1.x flag
   syntax would have been caught by pinning `syft: v0.x` and
   upgrading deliberately. Today the `download-syft` action picks a
   "current" version every run.

4. **Default to `shell: bash` on every multi-line `run:` block.** The
   two PowerShell parse errors were avoidable with a one-line fix on
   every affected step. Bake this into the workflow style.

5. **Use Python for cross-platform primitives.** `hashlib` for
   hashing, `pathlib` for paths, `subprocess` for shelling out. It
   is on every runner and has a stable interface. Avoids the
   `sha256sum`-on-Windows and `zip`-on-Windows classes of failure.

6. **Test cliff.toml locally before relying on it.** The two TOML
   errors (mixed tables, non-valid escapes) would have been caught
   by running `git-cliff --config cliff.toml --tag v0.0.0 --dry-run`
   on the develop machine. Add this to the pre-commit checklist.

## The 10 commits in order

1. `5ed7643` — fix(release): correct stage-artifact binary path
2. `2995947` — fix(release): SBOM step uses correct target/ path + syft v1.x flag
3. `60ae579` — fix(release): use Python for cross-platform SHA-256
4. `ef74e52` — fix(release): Windows uses tar.gz instead of zip
5. `995bf35` — fix(release): sign-artifact step uses shell: bash on Windows
6. `97ab5f9` — fix(release): SBOM step also uses shell: bash on Windows
7. `f4f7267` — fix(cliff): commit_parsers as array-of-tables, not mixed single/sub-tables
8. `4f27fe7` — fix(cliff): double non-valid backslash escapes in commit_parsers regexes
9. `4b786e4` — fix(release): install cosign in the publish job
10. `aaab474` — fix(release): set working-directory: artifacts on sign SHA256SUMS

## What is NOT covered here

- The `mcp-server` binary was NOT built or published. The matrix
  builds `omega-cli`; `mcp-server` is a separate sidecar binary.
  If `mcp-server` is meant to be a public artifact, the matrix needs
  a second build job for it, or a single matrix entry with multiple
  `binaries` and a fan-out step. Not done; the current 1-binary
  shape matches the README which describes `omega` as the CLI.
- No SBOM comparison or cosign verification was done in CI after
  publish. The release is signed; consumers need to run
  `cosign verify-blob` themselves. Consider adding a `verify`
  workflow that re-verifies on a schedule, so any post-publish
  keyless-signature issues get caught.
- The cosign identity for the keyless signature is whatever GitHub
  Actions OIDC produces. Document this in the release notes or
  `SECURITY.md` so consumers know what to pin against.

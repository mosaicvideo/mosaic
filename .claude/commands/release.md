---
description: Cut a new Mosaic release — preflight checks, commit pending WIP, write changelog + release notes, bump version, tag, push.
argument-hint: <version> [--dry-run]
---

You are running the Mosaic release pipeline. Follow these steps exactly, in order. Stop and ask the user if any step fails or if a precondition isn't met.

Arguments: **$ARGUMENTS**

Parse the arguments:
- First positional arg is the version (semver: `X.Y.Z` or `X.Y.Z-pre`).
- If `--dry-run` appears anywhere in the args, set `DRY_RUN=1` for the rest of this run. In dry-run mode, do everything locally but skip **both** `git push` calls. Announce dry-run mode clearly to the user at the start.

If the version is missing or not valid semver, stop and ask the user. Don't guess.

Cache these values once at the top of the run (don't recompute them mid-flight):
- `REPO_URL=$(git remote get-url origin | sed 's/\.git$//')` — base URL for compare links.
- `OWNER_REPO=$(gh repo view --json nameWithOwner -q .nameWithOwner)` — e.g. `mosaicvideo/mosaic`.
- `TODAY=$(date +%Y-%m-%d)` — real system date (do **not** use your internal date; it drifts).

## Step 1 — Resolve WIP

1. Run `git branch --show-current`. Must be `main`. Otherwise stop and ask.
2. Run `git status --short`. If clean, note it and skip to Step 2.
3. Show the user the WIP as a file-level summary, then draft a conventional-commit message inferred from the diff (`feat:` / `fix:` / `docs:` / `chore:` etc. — read the diff, don't guess from filenames). Show the drafted message to the user and **ask for approval or edits before committing**. Don't commit without an answer.
4. On approval: `git add <specific files>` (list them by name, don't use `git add -A`), commit with the approved message, then `git push origin main` (skip the push in dry-run).

## Step 2 — Preflight (blocking)

Run these checks in order. Any failure aborts the release.

1. **Branch:** already on `main` from Step 1.
2. **CHANGELOG has `[Unreleased]` section:** `grep -q '^## \[Unreleased\]' CHANGELOG.md`. If missing, abort with a clear message asking the user to add one.
3. **Tag doesn't exist locally:** `git tag -l v$VERSION` must be empty.
4. **Tag doesn't exist on remote:** `git ls-remote --tags origin refs/tags/v$VERSION` must be empty. If either tag check fails, abort — don't offer to force-delete; the user needs to investigate why a tag with this version already exists.
5. **Remote is in sync:** `git fetch origin` then `git rev-list HEAD..origin/main --count` must equal `0`. If not, abort — user needs to `git pull --rebase` and reconsider.
6. **Verification gate:** run all of these from the repo root. **Every one must succeed** — do not proceed if any fails.
   ```
   (cd src-tauri && cargo check --all-targets)
   (cd src-tauri && cargo clippy --all-targets -- -D warnings)
   (cd src-tauri && cargo test)
   (cd src-tauri && PATH="/opt/homebrew/opt/ffmpeg-full/bin:$PATH" cargo test --features test-api)
   ```
   The last command runs the integration tests that exercise real ffmpeg subprocesses. The `PATH` prefix is required on macOS because the default Homebrew `ffmpeg` bottle lacks libfreetype — the test suite needs `ffmpeg-full`. On Linux/Windows you can drop the `PATH` prefix; adapt if you're running this on a non-macOS machine.

   If any gate fails, abort. Do **not** offer to silence warnings, `--`-skip failing tests, or auto-fix — the user needs to decide how to handle real failures. (The v0.1.3 release had to be unwound once because the clippy gate was skipped; do not skip any gate.)
7. **Commits since last tag exist:** `git log $(git describe --tags --abbrev=0)..HEAD --oneline` must be non-empty. If there's nothing new, abort — no point tagging an empty release.

## Step 3 — Draft release notes

1. `PREV_TAG=$(git describe --tags --abbrev=0)`.
2. List commits since that tag: `git log $PREV_TAG..HEAD --pretty=format:'%h %s' --no-merges`.
3. Bucket each commit into **Added / Fixed / Changed / Removed** based on what actually changed for a user, not just the conventional-commit prefix. Content wins over prefix:
   - A `chore:` that drops an installer flag → **Changed** (or **Removed**).
   - A `feat:` that only adds a dev-only test fixture → drop entirely.
   - A `refactor:` that changes visible behavior → belongs in **Changed**.
   - `chore: bump version to X.Y.Z` → always drop from notes.
   For each commit whose subject doesn't clearly describe the user impact, read the diff (`git show <sha> --stat` then the relevant file diffs) before writing the bullet.
4. For each kept commit, write a plain-English, user-facing bullet. Explain the impact, not the implementation.
5. Show the user the full draft (all four buckets + any dropped commits with a one-line reason each). Iterate on their edits until they approve. **Don't touch any file until they approve.**

## Step 4 — Write the changelog entry

Edit `CHANGELOG.md`:

1. Under `## [Unreleased]`, insert: `## [$VERSION] - $TODAY`.
2. Populate with the approved buckets (omit empty ones).
3. Update the link-reference footer:
   - Change the `[unreleased]` line's compare URL from `$PREV_TAG...HEAD` to `v$VERSION...HEAD`.
   - Insert `[$VERSION]: $REPO_URL/compare/$PREV_TAG...v$VERSION` in the correct position (newest first).

## Step 5 — Rewrite release.yml `releaseBody`

Edit `.github/workflows/release.yml`:

1. Replace only the `releaseBody: |` block (and nothing outside it) with notes derived from the Step 3 draft, in the format already used:
   - `## What's New` heading.
   - `### Added` / `### Fixed` / `### Changed` subsections — same bullets as CHANGELOG, expanded slightly (bold lead phrase + explanation). These are intentionally richer than the CHANGELOG entries because they're the user's first-read when landing on the Releases page.
   - `### Requirements` — keep the current ffmpeg + MediaInfo block verbatim unless the actual requirements changed. If they did, the user should have flagged this in Step 3.
   - `### Notes` — update the Windows code-signing note if still accurate, and the "upgrading from vX.Y.Z?" pattern using `$PREV_TAG` as the reference.

## Step 6 — Bump, commit, tag

1. Run `node scripts/bump-version.mjs $VERSION` **without** `--tag`. This edits `package.json`, `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml`, and regenerates `src-tauri/Cargo.lock`.
2. Stage exactly these files:
   ```
   git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock CHANGELOG.md .github/workflows/release.yml
   ```
3. Commit: `git commit -m "chore: release v$VERSION"` (single commit containing the bump + the changelog + the releaseBody update — everything the tag should point at).
4. Tag: `git tag v$VERSION`.

## Step 7 — Push (confirmation gate)

Tag push triggers the release workflow and is externally visible to anyone watching the repo. **Get explicit confirmation from the user before pushing**, even in non-dry-run mode. Show them:
- The commits about to ship (`git log origin/main..HEAD --oneline`).
- The tag name.
- The Actions URL that will fire.

On confirmation (skip entirely in dry-run mode):
1. `git push origin main`
2. `git push origin v$VERSION`

If either push fails, stop and show the error — do **not** retry with force flags.

## Step 8 — Verify + hand off

1. `gh run list --workflow=release.yml --limit=1` to confirm the workflow queued.
2. Print to the user:
   - Actions run URL: `https://github.com/$OWNER_REPO/actions`.
   - Releases page URL: `https://github.com/$OWNER_REPO/releases`.
   - Reminder: the release is created as a **draft**. After the workflow finishes (~8 min), the user must open the release on GitHub, review the auto-filled notes against `CHANGELOG.md`, and click **Publish** — otherwise `/releases/latest/download/latest.json` won't resolve and the auto-updater can't find the new version.

## Dry-run summary

At the end of a dry-run, print a summary of:
- What would have been committed and pushed.
- The local tag that was created (advise the user to `git tag -d v$VERSION` and reset if they want to discard the dry-run artifacts).

## Failure policy

- Any failing command aborts. Do not retry destructive operations with force flags.
- Do not skip preflight checks "just this once."
- Do not silence clippy / check warnings; the user needs to see and decide.
- If you're unsure whether a bullet belongs in the release notes, ask — don't guess.

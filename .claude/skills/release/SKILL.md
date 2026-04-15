---
name: release
description: Prepare and publish a new Sapphire release — writes the CHANGELOG entry, bumps the version, commits, tags, and pushes.
---

You are preparing a new release of the Sapphire programming language. The project root is the current working directory.

The version to release is provided as the argument to this skill (e.g. `0.3.0`). If no version was provided, ask the user for one before proceeding.

## Steps

1. **Determine the previous release tag** by running:
   `git describe --tags --abbrev=0`

2. **Get all commits since that tag** by running:
   `git log <prev-tag>..HEAD --oneline`

3. **Read the existing CHANGELOG.md** to understand the format and style of previous entries.

4. **Write the changelog entry** for the new version. Follow these rules exactly:
   - Section header: `## v<version>`
   - Group commits into bold category headers (e.g. **Language**, **Bug fixes**, **Standard library**, **CLI**, **VM**) — only include categories that have changes
   - Each item is a single bullet starting with `-`
   - Use plain prose — no technical jargon, no commit hashes
   - Include a short code example (fenced code block) for any notable new syntax or language features
   - Omit internal refactoring, test-only changes, and benchmark additions unless they are user-visible
   - End the section with `---` on its own line (matching the existing separator style)

5. **Prepend the new entry** to CHANGELOG.md — insert it immediately after the `# Changelog` heading, before the previous `## v...` entry.

6. **Show the user the draft changelog entry** and ask them to confirm before proceeding. If they request changes, make them and show the updated entry again.

7. Once confirmed, **run the release script**:
   `bash scripts/release.sh <version>`

   This will:
   - Bump the version in `Cargo.toml`
   - Commit `Cargo.toml` and `CHANGELOG.md`
   - Create the git tag `v<version>`
   - Push the commit and tag (which triggers the GitHub Actions release workflow)

8. Report the result to the user.

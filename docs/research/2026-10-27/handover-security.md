# Handover: security audit (2026-10-27)

## Scope and result

The public repository `cristianonescu/OpenStitch` was audited in full: all 112 commits reachable
from every branch and tag (47 refs, merged and unmerged), with gitleaks 8.24.3, trufflehog 3.88.26
and a pattern grep (cloud keys, GitHub/Slack/OpenAI/Higgsfield-style tokens, private keys,
certificates, password/secret/token assignments, licence and activation keys), plus checks for
sensitive file paths ever committed, email addresses, home and local paths, machine names, PNG/SVG
metadata, readable strings in every embroidery fixture, the five workflows, and the repository's
GitHub security settings. Result: **no secret of any kind has ever been committed**; nothing to
rotate and no history rewrite needed. No third-party content that may not be redistributed:
reference PES files are the owner's own simple shapes or OpenStitch re-saves, fonts are OFL,
thread charts hold colour facts only, and there are no PE-DESIGN screenshots or Brother files.
The only personal-data items were local `D:\…` folder paths in `compat/reference/MANIFEST.json`
`settings` notes (low severity); PR #65 redacts them, adds a pinned-binary gitleaks job to
`rust.yml`, and extends `.gitignore` (`.env*`, keys and certificates, editor folders,
`.claude/worktrees/`). The full report with per-finding commits is outside the repo at
`<reference-notes>\security-audit.md`.

## Two settings for the owner to enable

Secret scanning and push protection are already on. In GitHub → Settings → Code security:

1. **Secret scanning → Non-provider patterns**: catches generic high-entropy keys and
   passwords that no provider pattern matches.
2. **Dependabot alerts** (and optionally security updates): currently disabled, so vulnerable
   crates in `Cargo.lock` are not reported.

## Rule for manifest notes (now in compat/reference/README.md)

`settings` and `notes` in `compat/reference/MANIFEST.json` are public. Write what was done in
PE-DESIGN, not where it was done: no local paths (`<home>`, `D:\…`), user names, machine
names or licence details. Name an input file by its repository path or its SHA-256. The
`secrets` CI job does not catch these, so reviewers check them by eye.

## Open item

PR #65 is last in the merge queue. On the coordinator's ping: rebase on origin/main, extend the
redaction to every `D:\…` fragment then in HEAD (including the two satin-auto-bend entries from
PR #56), keep the gitleaks job, `.gitignore` and README rule as they are, then stop once #65 is
merged.

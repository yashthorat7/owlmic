# Git Workflow, PR Guidelines & Agent Safety - Domain Skill

> **Scope**: Branch management, commits, staging discipline, pull requests, upstream synchronization, and AI agent guardrails.
> **Living Source of Truth**: Strictly synchronized with [`.github/CONTRIBUTING.md`](../../.github/CONTRIBUTING.md), [`scripts/hooks/commit-msg`](../../scripts/hooks/commit-msg), and [`scripts/hooks/pre-commit`](../../scripts/hooks/pre-commit).

---

## 1. Strict Read-Only Rule for AI Agents (Zero Exceptions)

AI agents operating in this repository must strictly adhere to the following safety boundary:

- **STRICTLY FORBIDDEN COMMANDS**:
  Agents are NEVER permitted to execute mutating or state-changing Git commands:
  `git add`, `git add .`, `git commit`, `git push`, `git pull`, `git fetch`, `git checkout -b`, `git branch`, `git switch`, `git merge`, `git rebase`, `git stash`, `git reset`, `git clean`, `git cherry-pick`.
  Agents must NEVER run `git add .` or any staging/commit/pull/merge commands by themselves.
- **ALLOWED READ-ONLY COMMANDS**:
  `git status`, `git diff`, and `git log` ONLY.
- **On-Demand Rule**:
  Agents must NEVER proactively append unsolicited `git add` or `git commit` commands to responses. Formulate Git commands **only when explicitly requested by the user** (e.g., *"give me a git command"*, *"how do I commit this?"*, *"create a branch and commit"*).

---

## 2. The Single Copy-Paste Rule (Separate Lines, No `&&`)

When formulating Git instructions for the user:

> [!IMPORTANT]
> **NEVER split commands across multiple snippets or markdown blocks, and DO NOT chain with `&&`.**  
> Always provide the entire sequence on **separate lines** inside a **single copy-pasteable code block**. The user can copy the block once and paste it into their terminal to execute cleanly line-by-line.

❌ **Bad (Fragmented across multiple chat blocks):**
```bash
git checkout -b refactor/verify
```
Then run:
```bash
git add .
```
Then run:
```bash
git commit -m "refactor(scripts): make verification compact"
```

❌ **Bad (Chained with `&&`):**
```bash
git checkout -b refactor/compact-verification && git add . && git commit -m "..."
```

✅ **Good (Single Copy-Paste Block, Separate Lines):**
```bash
git checkout -b refactor/compact-verification
git add .
git commit -m "refactor(scripts): make pre-push verification compact and legible"
git branch -f main origin/main
```

---

## 3. Semantic Branch Naming Specification

All branch names must strictly match the regex enforced by [`scripts/hooks/pre-commit`](../../scripts/hooks/pre-commit) and documented in [`.github/CONTRIBUTING.md`](../../.github/CONTRIBUTING.md):

```text
<type>/<short-kebab-description>
```

### Pattern Enforcement
- **Regex**: `^(feat|fix|docs|refactor|perf|test|chore|build|ci)/[a-z0-9-]+$`
- **Allowed Types**:
  - `feat`: New feature or user-facing functionality.
  - `fix`: Bug fix.
  - `docs`: Documentation updates.
  - `refactor`: Code refactoring without behavior changes.
  - `perf`: Performance optimization.
  - `test`: Adding or updating test suites.
  - `chore`: Tooling, scripts, configuration, dependency updates.
  - `build`: Build system or packaging changes.
  - `ci`: GitHub Actions workflows and CI scripts.
- **Naming Invariants**:
  - Strictly **lowercase alphanumeric and hyphens** (`[a-z0-9-]`).
  - No underscores (`_`), no camelCase, no uppercase letters, no trailing punctuation.
  - Short, clear, and descriptive kebab-case (2-4 words max).

### Examples
- ✅ `feat/speaker-loopback`
- ✅ `fix/mic-buffer-underrun`
- ✅ `refactor/compact-verification`
- ✅ `chore/clean-hooks`
- ✅ `docs/clarify-wire-spec`
- ❌ `Feature/Loopback_Fix` (uppercase, underscore, invalid type)
- ❌ `update` (missing type prefix and kebab description)
- ❌ `fix/audio.underrun` (period not allowed)

---

## 4. Conventional Commits & Strict 72-Character Hard Limit

Every commit message must strictly satisfy the format enforced by [`scripts/hooks/commit-msg`](../../scripts/hooks/commit-msg):

```text
<type>(<scope>): <lowercase imperative summary>
```

### Non-Negotiable Hard Limit: Maximum 72 Characters
- **The first line MUST be <= 72 characters total** (including `<type>(<scope>): `).
- Any commit exceeding 72 characters is **immediately rejected with exit code 1** by the git hook.
- Agents must actively count characters before presenting any commit message.

### Regex Pattern
```regex
^(feat|fix|perf|refactor|docs|test|ci|chore|build)\((android|desktop|pc|win|linux|proto|design|installer|scripts|ci|site|repo)\): [a-z0-9].{3,68}[^\.]$
```

### Allowed Scopes
- `android`: Android client app (`android/app`, `android/core`, `android/media`).
- `desktop`: Desktop companion core crates (`desktop/crates/*`).
- `win`: Windows-specific platform layer (`owlmic-vcam`, GDI UI, WASAPI).
- `linux`: Linux-specific backend (`owlmic-linux`, PipeWire, V4L2).
- `proto`: Wire protocol framing and test vectors (`shared/protocol/`).
- `design`: Design tokens, copy, settings schemas (`shared/design/`).
- `installer`: Windows installer or Linux packaging (`scripts/packaging/*`).
- `scripts`: Build, test, and verification automation (`scripts/*`).
- `ci`: GitHub Actions workflows (`.github/workflows/`).
- `site`: Website (`website/`).
- `repo`: Repository layout, multi-component refactoring, root files.

### Message Rules
1. **Summary starts lowercase or digit**: `[a-z0-9]` (e.g. `make verification compact`, NOT `Make verification compact`).
2. **Imperative mood**: Use `add`, `fix`, `make`, `clean`, `update` (NOT `added`, `fixes`, `updated`, `making`).
3. **No trailing period**: Never end the commit message with a period `.`.
4. **Length <= 72 characters**: Count the full string length including quotes.

### Character Count Calculation Examples
- `refactor(scripts): make pre-push verification compact and legible` -> **63 chars** (<= 72) ✅
- `fix(pc): prevent audio jitter buffer underflow on rapid link switch` -> **67 chars** (<= 72) ✅
- `chore(repo): update git workflow and branch naming guidelines` -> **60 chars** (<= 72) ✅
- `refactor(scripts): make verification thing show compact and concise while keeping everything` -> **92 chars** (EXCEEDS 72) ❌

---

## 5. Single Copy-Paste Command Blueprints by Scenario

Whenever the user asks for Git commands, choose the matching scenario and present the commands on **separate lines** inside a **single code block**:

### Scenario A: Working on an Existing Feature Branch (Stage & Commit)
When already on a dedicated semantic feature branch (e.g. `refactor/compact-verification`):
```bash
git add .
git commit -m "<type>(<scope>): <lowercase imperative summary, <=72 chars>"
```

### Scenario B: Working on an Existing Feature Branch (Stage, Commit & Push)
```bash
git add .
git commit -m "<type>(<scope>): <lowercase imperative summary, <=72 chars>"
git push origin <branch-name>
```

### Scenario C: Accidental Work on `main` (Branch, Stage, Commit & Reset Local `main`)
When working or uncommitted changes exist on `main`, this single block shifts all commits/changes to the semantic branch and resets local `main` to `origin/main` so `main` stays pristine:
```bash
git checkout -b <type>/<short-kebab-name>
git add .
git commit -m "<type>(<scope>): <lowercase imperative summary, <=72 chars>"
git branch -f main origin/main
```

### Scenario D: Accidental Work on `main` + Push to Fork
Same as Scenario C, but also pushes the new feature branch to personal fork with upstream tracking:
```bash
git checkout -b <type>/<short-kebab-name>
git add .
git commit -m "<type>(<scope>): <lowercase imperative summary, <=72 chars>"
git branch -f main origin/main
git push -u origin <type>/<short-kebab-name>
```

### Scenario E: Upstream Sync (`main` to `origin/main`)
To update local `main` from canonical upstream repository and publish it to the personal remote fork:
```bash
git checkout main
git pull upstream main
git push origin main
```

### Scenario F: Integrating / Merging Feature Branch into `main` and Pushing
```bash
git checkout main
git merge <type>/<short-kebab-name>
git push origin main
```

---

## 6. Pull Request (PR) Workflow & Standards

When the user requests to raise, pull, or prepare a PR (e.g., *"pull a PR"*, *"raise a PR"*, *"create a PR"*):

1. **Verify Branch**: Ensure the branch is not `main`. If on `main`, use **Scenario D** above first.
2. **PR Title**: Must conform strictly to Conventional Commits (max 72 chars):
   ```text
   <type>(<scope>): <high-level imperative summary>
   ```
3. **PR Creation**: Provide the GitHub CLI command:
   ```bash
   gh pr create --base main --head <type>/<short-kebab-name> --title "<type>(<scope>): <summary>" --body-file pr-description.md
   ```

### PR Description Template:
```markdown
## Summary
A concise 2-3 sentence overview of what this pull request introduces and why.

## Key Changes
- **Module / Path**: Bulleted summary of concrete architectural and code changes.
- **Deletions / Relocations**: Note moved or deleted obsolete files.

## Invariant & Quality Verification
- [x] Invariants verified: `python scripts/core/verify-repo-invariants.py` passed (0 errors).
- [x] Desktop Rust workspace: `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --workspace` passed.
- [x] Android client: `./gradlew testDebugUnitTest lintDebug` passed.
- [x] Binary ban enforced: Zero binaries committed.
```

---

## 7. Clean Repository & Staging Guardrails

1. **Strict Binary Ban**: Never stage or commit binaries (`.exe`, `.dll`, `.apk`, `.zip`, `.tar.gz`, `.so`, `.aab`).
2. **Pre-Push Invariant Check**: Run `python scripts/core/verify-repo-invariants.py` and `python scripts/core/check.py` before pushing.
3. **Clean Root Discipline**: Root directory contents are strictly fixed and minimal. Never place temporary or scratch files in root.

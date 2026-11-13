# Git Workflow, PR Guidelines & Agent Safety - Domain Skill

> **Scope**: Branch management, commits, staging discipline, pull requests, and AI agent guardrails.

---

## 1. Strict Read-Only Rule for AI Agents (Zero Exceptions)

AI agents operating in this repository must strictly adhere to the following safety boundary:

- **STRICTLY FORBIDDEN COMMANDS**:
  Agents are NEVER permitted to execute mutating or state-changing Git commands:
  `git add`, `git add .`, `git commit`, `git push`, `git checkout -b`, `git branch`, `git switch`, `git merge`, `git rebase`, `git stash`, `git reset`, `git clean`, `git cherry-pick`.
  Agents must NEVER run `git add .` or any staging/commit commands by themselves.
- **ALLOWED READ-ONLY COMMANDS**:
  `git status`, `git diff`, and `git log` ONLY.
- **On-Demand Rule**:
  Agents must NEVER proactively append unsolicited `git add` or `git commit` commands to responses. Formulate Git commands **only when explicitly requested by the user** (e.g., *"give me a git command"*, *"how do I commit this?"*).

---

## 2. Commit Generation Protocol (When Asked)

When the user asks to formulate or make a Git commit (e.g., *"make a commit"*, *"how do I commit this?"*, *"give me a git command"*):

1. **Inspect Historical Context**:
   Run `git log -1` (or `git log -n 5 --oneline`) to inspect the previous commit style, scope, and history.
2. **Inspect All Changes Since Last Commit**:
   Run `git status`, `git diff --stat`, and `git diff` (plus `git diff --staged`) to thoroughly review every modified, added, and deleted path.
3. **Convenient Staging in User Messages (`git add .`)**:
   - In response messages formulated for the user, output `git add .` as the convenient staging command so the user can quickly stage all changes.
   - **Agent Safety Rule**: `git add .` belongs in output messages for the user—agents must **never** run `git add .` by themselves.
   - When staging surgical or partial changes, explicit file/folder paths may be provided as an alternative.
4. **Formulate Concise, Context-Aware Conventional Commit**:
   Summarize all changes done since the last commit into a concise, accurate Conventional Commit message:
   ```bash
   git commit -m "<type>(<scope>): <short lowercase imperative summary, max 72 chars, no trailing period>"
   ```
   If changes span multiple distinct, unrelated concerns, offer cleanly grouped atomic commit instructions.

---

## 3. Conventional Commits Specification

```text
<type>(<scope>): <short lowercase imperative summary, max 72 chars, no trailing period>
```

### Allowed Types
- `feat`: New feature or user-facing functionality.
- `fix`: Bug fix.
- `perf`: Performance optimization.
- `refactor`: Code change that neither fixes a bug nor adds a feature.
- `docs`: Documentation changes only.
- `test`: Adding or correcting tests.
- `ci`: CI/CD pipeline or workflow changes.
- `chore`: Maintenance, dependency updates, tooling.
- `build`: Changes to build systems, packaging, or external manifests.

### Allowed Scopes
- `android`: Android client app (`android/app`, `android/core`, `android/media`).
- `desktop`: Desktop companion core crates (`desktop/crates/*`).
- `win`: Windows-specific platform layer (`owlmic-vcam`, GDI UI, WASAPI).
- `linux`: Linux-specific backend (`owlmic-linux`, PipeWire, V4L2).
- `proto`: Wire protocol framing and test vectors (`shared/protocol/`).
- `design`: Design tokens, copy, settings schemas (`shared/design/`).
- `installer`: Windows installer or Linux packaging (`scripts/packaging/*`).
- `scripts`: Build and verification automation (`scripts/*`).
- `ci`: GitHub Actions workflows (`.github/workflows/`).
- `site`: Website (`website/`).
- `repo`: Repository layout, multi-component refactoring, root files.

---

## 4. Pull Request (PR) Workflow & Standards

When the user requests to raise, pull, or prepare a PR (e.g., *"pull a PR"*, *"raise a PR"*, *"create a PR"*):

### 1. Branch Verification & Accidental `main` Detection
- Always inspect the current branch using `git branch --show-current` or `git status`.
- **NEVER submit a PR directly from `main`**.
- If the user is currently on a dedicated feature branch (`<type>/<short-kebab-name>`), verify all working tree changes are committed, push the branch (`git push -u origin <branch>`), and proceed to generate the PR.

### 2. Smart Branch Migration (When Accidentally on `main`)
If the user has been working or committing directly on `main` without creating a branch first:
All changes and commits made since the last clean upstream state (`origin/main`) **must be safely shifted to a new dedicated branch**, and local `main` must be reset so it stays clean.

Formulate the following step-by-step instructions for the user:

#### Step A: Sync Upstream Base
Ensure the local repository is aware of the upstream tracking state:
```bash
git fetch origin main
```

#### Step B: Create and Switch to Semantic Feature Branch
Branch off directly from the current state. This carries over all commits made on `main` as well as any uncommitted working tree changes:
```bash
git checkout -b <type>/<short-kebab-name>
```
*(Example: `git checkout -b feat/android-camera-stream` or `git checkout -b chore/reduce-repo-size`)*

#### Step C: Commit Any Remaining Changes (if applicable)
If there are uncommitted working tree changes, stage and commit them on the new branch:
```bash
git add .
git commit -m "<type>(<scope>): <concise imperative summary>"
```

#### Step D: Reset Local `main` Cleanly
While remaining on the new feature branch, safely reset local `main` back to upstream `origin/main` without affecting the current branch or working tree:
```bash
git branch -f main origin/main
```

#### Step E: Push Feature Branch to Remote
Push the new feature branch to origin with tracking:
```bash
git push -u origin <type>/<short-kebab-name>
```

---

### 3. PR Title
Only once the branch is clean and pushed, formulate the PR title conforming strictly to Conventional Commits:
```text
<type>(<scope>): <high-level imperative summary>
```

### 4. PR Creation & Description Template
Formulate the GitHub CLI command (or web URL) and provide the complete PR description:

```bash
gh pr create --base main --head <type>/<short-kebab-name> --title "<type>(<scope>): <summary>" --body-file pr-description.md
```
*(Or guide the user to open a PR on GitHub comparing `main` ← `<type>/<short-kebab-name>`)*

#### PR Description Template:
```markdown
## Summary
A concise 2-3 sentence overview of what this pull request introduces and why.

## Key Changes
- **Module / Path**: Bulleted summary of concrete architectural and code changes.
- **Deletions / Relocations**: Note moved or deleted obsolete files.

## Invariant & Quality Verification
- [x] Invariants verified: `python scripts/verify-repo-invariants.py` passed (0 errors).
- [x] Desktop Rust workspace: `cargo check`, `cargo test`, `cargo clippy`, `cargo fmt` passed.
- [x] Android client: `./gradlew testDebugUnitTest lintDebug` passed.
- [x] Website: `npm run build` passed.
- [x] Binary ban enforced: Zero binaries committed.
```

---

## 5. Clean Repository & Staging Guardrails

1. **Binary Ban**: Never stage or commit binaries (`.exe`, `.dll`, `.apk`, `.zip`, `.tar.gz`, `.so`).
2. **Pre-Commit Verification**: Run `python scripts/verify-repo-invariants.py` and `./scripts/check.sh` (or `.\scripts\check.ps1`) before pushing.
3. **Clean Root Discipline**: Do not add ad-hoc or temporary files to the repository root.

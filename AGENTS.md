# Owlmic AI Agent Instructions

> `CLAUDE.md` imports this file via `@AGENTS.md`.  
> The documentation in [`docs/README.md`](docs/README.md) is the living source of truth for architecture and specifications.

Owlmic is a real-time, peer-to-peer Continuity Microphone, Camera, and Speaker Loopback system for **Android (client)**, **Windows (server)**, and **Linux (server)**.

---

## 1. Mandatory Pre-Task Context Routing & Priority Tiers

All repository skills follow the 3-tier operational priority model defined in [`docs/skills/README.md`](docs/skills/README.md):

### Priority 1: Core Output, Clean Code & Token Economy (Always Active)
Enforced across every prompt turn, inspection, and response:
- **Writing / Refactoring Code (Anti-Slop & Lean Code):** Read [`docs/skills/anti-slop-and-lean-code.md`](docs/skills/anti-slop-and-lean-code.md)
- **Context Economy & Agent Discipline:** Read [`docs/skills/token-saving-and-context.md`](docs/skills/token-saving-and-context.md)
- **Executing Procedures & Quality Verification:** Read [`docs/skills/procedures-and-verification.md`](docs/skills/procedures-and-verification.md)

### Priority 2: Subsystem & Platform Development (Active on Modification)
Load only the specific domain rulebook matching the modified path:
- **Touching Desktop Companion (`desktop/` - Rust Core, Audio DSP, Transporter):** Read [`docs/skills/desktop-rust.md`](docs/skills/desktop-rust.md)
- **Touching Windows Platform (`owlmic-windows`, `owlmic-vcam`):** Read [`docs/skills/windows-backend.md`](docs/skills/windows-backend.md)
- **Touching Linux Backend (`owlmic-linux`, PipeWire, V4L2):** Read [`docs/skills/linux-backend.md`](docs/skills/linux-backend.md)
- **Touching `android/` (Kotlin, Compose, Oboe, CameraX):** Read [`docs/skills/android-kotlin.md`](docs/skills/android-kotlin.md) (no APK builds unless asked)
- **Touching Media Pipelines (`owlmic-media`, `android/media`):** Read [`docs/skills/media-and-dsp.md`](docs/skills/media-and-dsp.md)
- **Touching `shared/` (Protocol, Design, Assets, Vendor):** Read [`docs/skills/protocol-design-sync.md`](docs/skills/protocol-design-sync.md)

### Priority 3: On-Demand & Specialized Operations (Explicit Call Only)
Least priority; load only when explicitly requested:
- **Committing / Git Actions (On-Demand Only):** Read [`docs/skills/git-workflow.md`](docs/skills/git-workflow.md)
- **Building, Packaging & Developer Menu (`scripts/menu.py`, `scripts/core/build.py`, `releases/`):** Read [`docs/skills/build-and-bundle.md`](docs/skills/build-and-bundle.md)
- **Touching Website (`website/`):** Read [`docs/skills/website.md`](docs/skills/website.md)

---

## 2. Non-Negotiable Global Invariants

1. **Product Name Casing**: Strictly `Owlmic` (or `owlmic` in lowercase identifiers). Never camelCase or split the product name.
2. **Role Invariant**: Phone is ALWAYS the client (`OwlmicService`); PC is ALWAYS the server (`TCP :7653`, `UDP :7654/:7655`).
3. **Anti-AI Slop & Lean Code**: Build strictly what is requested. Zero speculative abstractions, zero chatty comments, zero code churn. Adhere strictly to [`docs/skills/anti-slop-and-lean-code.md`](docs/skills/anti-slop-and-lean-code.md).
4. **Single Source of Truth**: Never hardcode colors, UI copy, or settings defaults. Read from `shared/design/tokens.json`, `shared/design/copy.json`, and `shared/design/settings.json`.
5. **Clean Root Discipline**: Root directory is fixed and minimal (`.github/`, `android/`, `desktop/`, `docs/`, `scripts/`, `shared/`, `website/`, `AGENTS.md`, `CLAUDE.md`, `LICENSE`, `README.md`, `.editorconfig`, `.gitignore`, `.gitmodules`).
6. **Strict Binary Ban & On-Demand Packaging**: Never commit or stage compiled binaries (`.apk`, `.exe`, `.aab`, `.dll`, `.so`, `.zip`). Never build Android APKs during development or verification; build APKs only when explicitly requested by the user.
7. **Pre-Completion Verification**: Follow [`docs/skills/procedures-and-verification.md`](docs/skills/procedures-and-verification.md). Always run `python scripts/core/verify-repo-invariants.py` and master checker `python scripts/core/check.py` before finishing.

---

## 3. Git Workflow & Safety Guardrails

All Git practices, PR standards, and agent rules are fully detailed in [`docs/skills/git-workflow.md`](docs/skills/git-workflow.md). Key mandates:

1. **Strict Read-Only Agent Rule**: Agents NEVER execute state-changing Git commands (`git add`, `git add .`, `git commit`, `git push`, `git pull`, `git fetch`, `git checkout`, `git branch`, `git merge`, `git rebase`, etc.) by themselves. ONLY `git status`, `git diff`, and `git log` are permitted.
2. **On-Demand Only**: Never output unsolicited Git commands. Formulate Git commands only when explicitly requested by the user.
3. **Single Copy-Paste Command Block (Separate Lines, No `&&`)**: Never separate commands across multiple markdown snippets or steps. Output the entire sequence on **separate lines** inside a **single copy-paste code block** (do NOT use `&&`) so the user can copy once and run it cleanly in their terminal.
4. **Strict Branch Naming Standard**: Follow `.github/CONTRIBUTING.md` and `scripts/hooks/pre-commit`: `<type>/<short-kebab-description>` where `<type>` is strictly in `{feat, fix, docs, refactor, perf, test, chore, build, ci}` and the description is lowercase alphanumeric kebab-case (`[a-z0-9-]+`).
5. **Strict Conventional Commits & <= 72 Chars**: Follow `.github/CONTRIBUTING.md` and `scripts/hooks/commit-msg`: `<type>(<scope>): <lowercase imperative summary>`. Valid scopes: `{android, desktop, pc, win, linux, proto, design, installer, scripts, ci, site, repo}`. The first line MUST be strictly <= 72 characters total (hard hook limit), start lowercase, and have no trailing period.
6. **PR Creation & Smart Branch Migration**: When asked to raise/pull a PR, verify the branch. Never submit a PR from `main`. If on `main`, formulate instructions to shift all commits/changes to a dedicated semantic branch (`git checkout -b <type>/<short-kebab-name>`), reset local `main` to `origin/main` (`git branch -f main origin/main`), push the feature branch, and provide the PR title and markdown description. See [`docs/skills/git-workflow.md`](docs/skills/git-workflow.md).
7. **Upstream Sync & Main Maintenance**: When asked to sync upstream (`git pull upstream main`), publish to fork (`git push origin main`), or merge branches, formulate the exact single-block command sequence for the user. Agents never run mutating Git commands.

---

## 4. Communication & Output Style

1. **Direct, Friendly & Neat (Like a Friend, No Bluff)**:
   - Talk like a sharp, reliable teammate: casual yet precise, clear, and honest.
   - Zero fluff, zero filler, zero bluff. If something broke or is uncertain, call it out directly.
   - Format answers cleanly with concise bullets and clear next steps.
2. **High Signal, Zero Corporate Boilerplate**:
   - Skip generic sycophancy or filler introductions (*"Certainly! I would be delighted to..."*).
   - Link directly to affected lines and files using markdown file links (`[file.rs](file:///path/to/file.rs#L10-L25)`).

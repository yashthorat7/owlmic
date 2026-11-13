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
- **Touching Desktop Companion (`desktop/` - Rust Core, Audio DSP, Transporter):** Read [`docs/skills/pc-rust.md`](docs/skills/pc-rust.md)
- **Touching Windows Platform (`owlmic-windows`, `owlmic-vcam`):** Read [`docs/skills/windows-backend.md`](docs/skills/windows-backend.md)
- **Touching Linux Backend (`owlmic-linux`, PipeWire, V4L2):** Read [`docs/skills/linux-backend.md`](docs/skills/linux-backend.md)
- **Touching `android/` (Kotlin, Compose, Oboe, CameraX):** Read [`docs/skills/android-kotlin.md`](docs/skills/android-kotlin.md) (no APK builds unless asked)
- **Touching Media Pipelines (`owlmic-media`, `android/media`):** Read [`docs/skills/media-and-dsp.md`](docs/skills/media-and-dsp.md)
- **Touching `shared/` (Protocol, Design, Assets, Vendor):** Read [`docs/skills/protocol-design-sync.md`](docs/skills/protocol-design-sync.md)

### Priority 3: On-Demand & Specialized Operations (Explicit Call Only)
Least priority; load only when explicitly requested:
- **Committing / Git Actions (On-Demand Only):** Read [`docs/skills/git-workflow.md`](docs/skills/git-workflow.md)
- **Building, Packaging & Developer Menu (`menu.py`, `scripts/build.py`, `dist/`):** Read [`docs/skills/build-and-bundle.md`](docs/skills/build-and-bundle.md)
- **Touching Website (`website/`):** Read [`docs/skills/web-landing.md`](docs/skills/web-landing.md)

---

## 2. Non-Negotiable Global Invariants

1. **Product Name Casing**: Strictly `Owlmic` (or `owlmic` in lowercase identifiers). Never camelCase or split the product name.
2. **Role Invariant**: Phone is ALWAYS the client (`OwlmicService`); PC is ALWAYS the server (`TCP :7653`, `UDP :7654/:7655`).
3. **Anti-AI Slop & Lean Code**: Build strictly what is requested. Zero speculative abstractions, zero chatty comments, zero code churn. Adhere strictly to [`docs/skills/anti-slop-and-lean-code.md`](docs/skills/anti-slop-and-lean-code.md).
4. **Single Source of Truth**: Never hardcode colors, UI copy, or settings defaults. Read from `shared/design/tokens.json`, `shared/design/copy.json`, and `shared/design/settings.json`.
5. **Clean Root Discipline**: Root directory is fixed and minimal (`.github/`, `android/`, `desktop/`, `docs/`, `scripts/`, `shared/`, `website/`, `AGENTS.md`, `CLAUDE.md`, `LICENSE`, `README.md`, `menu.py`, `.editorconfig`, `.gitignore`, `.gitmodules`).
6. **Strict Binary Ban & On-Demand Packaging**: Never commit or stage compiled binaries (`.apk`, `.exe`, `.aab`, `.dll`, `.so`, `.zip`). Never build Android APKs during development or verification; build APKs only when explicitly requested by the user.
7. **Pre-Completion Verification**: Follow [`docs/skills/procedures-and-verification.md`](docs/skills/procedures-and-verification.md). Always run `python scripts/verify-repo-invariants.py` and master checker `python scripts/check.py` before finishing.

---

## 3. Git Workflow & Safety Guardrails

All Git practices, PR standards, and agent rules are fully detailed in [`docs/skills/git-workflow.md`](docs/skills/git-workflow.md). Key mandates:

1. **Strict Read-Only Agent Rule**: Agents NEVER execute state-changing Git commands (`git add`, `git add .`, `git commit`, `git push`, `git checkout`, `git branch`, `git merge`, etc.) by themselves. ONLY `git status`, `git diff`, and `git log` are permitted.
2. **On-Demand Commit Generation**: Never output unsolicited commit suggestions. Only formulate Git commands when explicitly requested by the user.
3. **Convenient Staging in User Messages (`git add .`)**: When generating commit instructions, formulate `git add .` in the response message for the user to conveniently stage all workspace changes. Agents must NEVER execute `git add .` by themselves.
4. **Context-Aware Commits**: When asked to formulate or make a commit, inspect changes since the last commit (`git status`, `git diff`), provide `git add .` in instructions for the user, and craft concise Conventional Commit messages (max 72 chars).
5. **PR Creation & Smart Branch Migration**: When asked to raise/pull a PR, verify the branch. Never submit a PR from `main`. If on `main` (even with commits already made on `main`), formulate instructions to safely shift all commits/changes to a dedicated semantic branch (`git checkout -b <type>/<short-name>`), reset local `main` to `origin/main` (`git branch -f main origin/main`), push the feature branch, and only then generate the PR title and markdown description. See [`docs/skills/git-workflow.md`](docs/skills/git-workflow.md).

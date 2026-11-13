# Owlmic AI Agent Instructions

> `CLAUDE.md` imports this file via `@AGENTS.md`.  
> The documentation in [`docs/README.md`](docs/README.md) is the living source of truth for architecture and specifications.

Owlmic is a real-time, peer-to-peer Continuity Microphone, Camera, and Speaker Loopback system for **Android (client)**, **Windows (server)**, and **Linux (server)**.

---

## 1. Mandatory Pre-Task Context Routing

Before modifying any file, inspect the corresponding domain rulebook:
- **Touching `pc/` (Rust Core, Audio DSP, Transporter):** Read [`docs/agents/pc-rust.md`](docs/agents/pc-rust.md)
- **Touching `pc/crates/owlmic-linux` (PipeWire, V4L2, Tray):** Read [`docs/agents/linux-backend.md`](docs/agents/linux-backend.md)
- **Touching `android/` (Kotlin, Compose, Oboe, CameraX):** Read [`docs/agents/android-kotlin.md`](docs/agents/android-kotlin.md)
- **Touching `protocol/` or `design/`:** Read [`docs/agents/protocol-design-sync.md`](docs/agents/protocol-design-sync.md)

---

## 2. Non-Negotiable Global Invariants

1. **Product Name Casing**: Strictly `Owlmic` (or `owlmic` in lowercase identifiers). Never camelCase or split the product name.
2. **Role Invariant**: Phone is ALWAYS the client (`OwlmicService`); PC is ALWAYS the server (`TCP :7653`, `UDP :7654/:7655`).
3. **Anti-AI Slop & Lean Code**: Build strictly what is requested. Zero speculative abstractions, zero chatty comments, zero code churn.
4. **Single Source of Truth**: Never hardcode colors, UI copy, or settings defaults. Read from `design/tokens.json`, `design/copy.json`, and `design/settings.json`.
5. **Clean Root Discipline**: Root directory is fixed and minimal (`.github/`, `android/`, `assets/`, `design/`, `docs/`, `landing/`, `pc/`, `protocol/`, `scripts/`, `AGENTS.md`, `CLAUDE.md`, `LICENSE`, `README.md`, `.editorconfig`, `.gitattributes`, `.gitignore`, `.gitmodules`).
6. **Strict Binary Ban**: Never commit or stage compiled binaries (`.apk`, `.exe`, `.aab`, `.dll`, `.so`, `.zip`).
7. **Pre-Completion Verification**: Always run `python scripts/verify-repo-invariants.py` and master checker `./scripts/check.sh` (or `.\scripts\check.ps1`) before finishing.

---

## 3. Git Workflow & Safety Guardrails

### Strict Read-Only Rule for Agents (Zero Exceptions)
- **NEVER execute ANY state-changing or mutating Git command**:
  `git add`, `git commit`, `git push`, `git checkout -b`, `git branch`, `git switch`, `git merge`, `git rebase`, `git stash`, `git reset`, `git clean`, `git cherry-pick` are **STRICTLY FORBIDDEN**.
- **ONLY read-only inspection commands are allowed**: `git status`, `git diff`, and `git log` ONLY.
- **Formulate commands for the user**: Even if asked to commit or push, return the exact command lines, branch names, and commit messages for the user to inspect and run themselves.

### Conventional Commits Format
```text
<type>(<scope>): <short lowercase imperative summary, max 72 chars, no trailing period>
```
- **Allowed Types**: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`, `chore`, `build`.
- **Allowed Scopes**: `android`, `pc`, `win`, `linux`, `proto`, `design`, `installer`, `scripts`, `ci`, `site`.

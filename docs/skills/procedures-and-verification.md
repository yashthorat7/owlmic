# Standard Operating Procedures & Verification - Domain Skill

> **Scope**: Development workflows, verification gates, quality assurance, defect resolution, and release readiness.

---

## 1. The 5-Stage Task Execution Lifecycle

Every code change, bug fix, or feature addition in Owlmic must proceed through five distinct phases:

```text
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ 1. Discovery &  │ ──► │ 2. Single-Source│ ──► │ 3. Surgical &   │
│ Context Routing │     │    Sync Check   │     │ Lean Code Edit  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                         │
                                                         ▼
┌─────────────────┐                             ┌─────────────────┐
│ 5. Safe Git     │ ◄────────────────────────── │ 4. Pre-Flight   │
│ Handoff         │   (Only on User Request)    │ Verification    │
└─────────────────┘                             └─────────────────┘
```

### Stage 1: Discovery & Context Routing
1. Check repository state using read-only `git status`. Confirm clean working tree or identify active modifications.
2. Route directly to the matching domain skill in `docs/skills/` (see [`AGENTS.md`](../../AGENTS.md)).
3. Strictly adhere to the **Read-Only Git Agent Rule**: Never execute mutating Git commands (`git add`, `git commit`, `git checkout`, etc.).

### Stage 2: Single-Source-of-Truth Sync Check
1. If touching wire messaging or framing: verify and synchronize `shared/protocol/vectors/*.json`.
2. If touching UI colors, geometry, or typography: verify `shared/design/tokens.json`.
3. If touching UI strings: verify `shared/design/copy.json` and ensure product name invariant (`Owlmic` / `owlmic`).
4. If touching user preferences: verify `shared/design/settings.json`.

### Stage 3: Surgical & Lean Implementation
1. Adhere to [`anti-slop-and-lean-code.md`](anti-slop-and-lean-code.md): zero chatty comments, zero speculative abstractions, zero code churn.
2. Adhere to [`token-saving-and-context.md`](token-saving-and-context.md): use surgical targeted edits (`edit_file`) instead of whole-file overwrites.
3. Obey clean root discipline: root directory contents are strictly fixed and minimal. Never place temporary or scratch files in root.

### Stage 4: Mandatory Pre-Flight Verification Gate
Run automated verification before declaring any task complete:

```bash
# 1. Master Repository Invariants (Required on every change)
python scripts/verify-repo-invariants.py

# 2. Rust Workspace (When touching desktop/ or shared/)
cargo fmt --all --check --manifest-path desktop/Cargo.toml
cargo clippy --workspace --all-targets --manifest-path desktop/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path desktop/Cargo.toml

# 3. Android Client (When touching android/ - unit tests & lint only; do NOT build APKs)
cd android
./gradlew testDebugUnitTest lintDebug
# Note: Never run assembleDebug or build APKs during verification; build APKs only when explicitly requested.

# 4. Website (When touching website/)
cd website
npm run lint && npm run build

# 5. Unified Developer Menu & Multi-Check Runner
python menu.py check      # Full pre-flight verification across platforms
python menu.py verify     # Repository invariants check only
```

### Stage 5: Safe Git Handoff (On-Demand Only)
1. Never emit unsolicited Git commands.
2. When explicitly requested by the user (*"give me a commit command"*, *"how do I commit this?"*):
   - Inspect all changes (`git status`, `git diff`).
   - Ban `git add .` or `git add -A`. Provide explicit paths: `git add <file1> <file2>`.
   - Craft a concise Conventional Commit (`feat(pc): ...`, `fix(android): ...`).
   - If on `main`, formulate smart branch migration instructions per [`git-workflow.md`](git-workflow.md).

---

## 2. Defect Resolution & Debugging Protocol

When addressing bugs, unexpected errors, or test failures:

1. **Reproduce First**: Formulate a reproducible unit test or minimal vector test before altering implementation code.
2. **Find the Root Cause**: Trace the issue to the exact invariant violation or protocol misalignment. Never apply speculative "band-aids" or suppress errors with empty catch blocks.
3. **Verify Zero Regressions**: Re-run the full verification suite to ensure fixes do not degrade latency budgets or break cross-platform synchronization.

# Owlmic Domain Skills Catalog

This directory contains modular domain rules and engineering skills guiding AI agents and human contributors when working on specific subsystems of the Owlmic repository.

The catalog is organized into **three operational priority tiers**:

```text
Task Request
     │
     ├──► Priority 1: Core Output & Token Economy (ALWAYS ACTIVE)
     │      ├─ anti-slop-and-lean-code.md  (Zero chatty comments, YAGNI, zero churn)
     │      ├─ token-saving-and-context.md (Surgical reads/edits, compact responses)
     │      └─ procedures-and-verification.md (Invariants, lint, clippy, unit tests)
     │
     ├──► Priority 2: Subsystem Development (ACTIVE ON MODIFICATION)
     │      ├─ desktop-rust.md / windows-backend.md / linux-backend.md
     │      ├─ android-kotlin.md / media-and-dsp.md
     │      └─ protocol-design-sync.md
     │
     └──► Priority 3: On-Demand & Specialized Operations (EXPLICIT CALL ONLY)
            ├─ git-workflow.md    (Read-only agent, explicit staging on-demand)
            ├─ build-and-bundle.md (Developer menu & release builder: menu.py, build.py)
            └─ website.md         (Marketing & product website)
```

---

## Priority 1: Core Output, Clean Code & Token Economy (Always Active)

Universal rules enforced across **every prompt turn, code generation, and inspection**:

| Skill File | Scope | Practical Directives |
|---|---|---|
| **[`anti-slop-and-lean-code.md`](anti-slop-and-lean-code.md)** | All code changes | Zero chatty comments, zero speculative abstractions, zero code churn, lean YAGNI implementations. |
| **[`token-saving-and-context.md`](token-saving-and-context.md)** | Agent operations & prompts | Surgical line-range file reading, targeted `edit_file` (no full overwrites), dense factual replies. |
| **[`procedures-and-verification.md`](procedures-and-verification.md)** | Execution lifecycle | 5-stage task lifecycle, mandatory pre-flight verification gate (`verify-repo-invariants.py`). |

---

## Priority 2: Subsystem & Platform Development (Active on Modification)

Targeted context rulebooks loaded **only when touching files in the corresponding subsystem**:

| Skill File | Target Path / Subsystem | Core Technical Invariants |
|---|---|---|
| **[`desktop-rust.md`](desktop-rust.md)** | `desktop/crates/*` | Standard blocking threads, bounded channels, Tokio banned outside Linux `bt.rs`. |
| **[`windows-backend.md`](windows-backend.md)** | `desktop/crates/owlmic-windows/`, `desktop/crates/owlmic-vcam/` | WASAPI virtual mic & loopback, Win 11 Media Foundation, Direct2D OLED tray. |
| **[`linux-backend.md`](linux-backend.md)** | `desktop/crates/owlmic-linux/` | PipeWire virtual source, v4l2loopback frame feeder, `ksni` tray, systemd user service. |
| **[`android-kotlin.md`](android-kotlin.md)** | `android/app/`, `android/core/` | Jetpack Compose UI, foreground service lifecycle, zero heap alloc on Oboe JNI threads, no APK builds unless asked. |
| **[`media-and-dsp.md`](media-and-dsp.md)** | `desktop/crates/owlmic-media/`, `android/media/` | 48 kHz standard, 10 ms cadence, cubic resampler ($\pm 0.2\%$), H.264 MTU slicing ($\le 1200\text{ B}$). |
| **[`protocol-design-sync.md`](protocol-design-sync.md)** | `shared/protocol/`, `shared/design/` | Protocol v3 framing, vector test lockstep, single source of truth for tokens & copy. |

---

## Priority 3: On-Demand & Specialized Operations (Explicit Call Only)

Least priority; kept out of context until explicitly requested by the user:

| Skill File | Scope / Trigger | Core Invariants |
|---|---|---|
| **[`git-workflow.md`](git-workflow.md)** | Version control & PRs (on user request only) | Strict read-only agent rule, convenient `git add .` staging, on-demand Conventional Commits only. |
| **[`build-and-bundle.md`](build-and-bundle.md)** | Developer menu & build system (`menu.py`, `scripts/build.py`) | Interactive developer menu, release binaries, installer packaging, or managing `dist/`. |
| **[`website.md`](website.md)** | Website (`website/`) | React 19, TypeScript, Vite, zero telemetry, pure OLED tokens, lightweight bundle. |

---

## Pre-Task Context Routing Rule

Agents must **never load the entire skills directory or full technical specs** into context simultaneously.
1. Apply Priority 1 rules universally.
2. Inspect the path to be touched, and load **only** the single matching Priority 2 subsystem skill (~300–770 tokens).
3. Access Priority 3 skills only when explicitly instructed.
4. Run `python scripts/verify-repo-invariants.py` before concluding.

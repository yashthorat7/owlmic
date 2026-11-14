# Token Economy & Context Efficiency - Domain Skill

> **Scope**: AI agent operations, context engineering, file inspection, editing patterns, and communication density.

---

## 1. Context Engineering & The Token Economy

Context windows and model attention are finite resources. Bloating the prompt with unnecessary file dumps, verbose explanations, or un-scoped rules degrades model reasoning, introduces hallucinations, and wastes tokens.

### Core Rules for Context Hygiene

1. **Targeted Pre-Task Routing**:
   - Only load the single domain skill required for the active subsystem (e.g., if editing `android/`, do not inspect `linux-backend.md` or `desktop-rust.md`).
   - Keep conversation context focused on the active problem domain.
2. **Surgical File Reading**:
   - **Never read entire 500+ line files** when inspecting a specific function, struct, or configuration block.
   - Use `StartLine` and `EndLine` to read only the target snippet.
   - Locate targets using specific text searches or shell grep (`Select-String` / `grep`) before opening files.
3. **Surgical File Editing**:
   - Always prefer `edit_file` with concise, unique `Target` and `Replacement` blocks.
   - **Never use `write_file` to rewrite an existing file** unless creating a brand-new file or replacing a trivial (< 20 line) stub. Rewriting full files consumes massive input/output tokens and increases risk of introducing accidental regressions.

---

## 2. High-Signal, Zero-Waste Communication

1. **Eliminate Conversational Filler**:
   - Strip polite boilerplate (e.g., *"Certainly! I would be delighted to help you with that task. Here is the explanation..."*).
   - State the action taken, key findings, and next steps directly and concisely.
2. **No Full-File Echoing**:
   - When modifying code, never dump the entire modified file back in chat.
   - Provide concise line references or clickable markdown file links (`[file.rs](file:///path/to/file.rs#L10-L25)`).
3. **Artifact Economy**:
   - When creating or updating markdown artifacts, **never re-summarize or duplicate artifact contents in chat**.
   - Point the user directly to the artifact and highlight only open decisions or critical verification results.
4. **Tool Execution Discipline**:
   - Avoid redundant tool invocations (e.g., repeatedly querying directory contents or running consecutive status commands).
   - Run combined checks or targeted commands with appropriate timeouts.

---

## 3. Best Practices Checklist

| Action | Wasteful / Token-Heavy (Avoid) | Efficient / Token-Lean (Enforce) |
|---|---|---|
| **Reading Code** | Reading 800 lines of `media.rs` | Reading lines 40–85 using `StartLine=40, EndLine=85` |
| **Editing Code** | Overwriting `LinkHub.kt` (400 lines) via `write_file` | Editing 6 lines via `edit_file` with precise target block |
| **Communicating** | Pasting 150 lines of code in markdown response | Showing 5-line diff excerpt and file link with line numbers |
| **Context Loading** | Reading all files in `docs/skills/` at once | Reading only the single domain skill triggered by file path |

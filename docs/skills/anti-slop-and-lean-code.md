# Anti-Slop & Lean Code Engineering - Domain Skill

> **Scope**: Repository-wide coding discipline across Rust, Kotlin, C++, TypeScript, and Python.

---

## 1. The Anti-AI Slop Mandate

Owlmic is a high-performance, real-time media bridge where latency budgets are measured in microseconds and reliability is paramount. Bloated abstractions, speculative code, and chatty comments degrade performance, increase maintenance overhead, and dilute code clarity.

### Banned Practices (Zero Tolerance)

| Anti-Pattern | Description | Correct Practice |
|---|---|---|
| **Chatty Comments** | Comments explaining self-evident statements (e.g., `// loop through items`, `// set status to true`, `// handle the error`). | Code must be self-documenting through precise naming and structure. Only explain *why* non-obvious real-time trade-offs exist. |
| **Speculative Abstractions** | Writing abstract classes, traits, factories, or generics "in case we need them in the future". | **YAGNI (You Aren't Gonna Need It)**. Implement strictly what is required for the concrete feature today. |
| **Boilerplate Placeholders** | Leaving `// TODO: implement later`, stubbed pass-throughs, or unimplemented mock functions. | Build complete, working implementations or do not commit the code. |
| **Code Churn** | Reformatting unrelated lines, changing variable names across an entire file, or rearranging imports. | Make minimal, surgical edits confined strictly to the functional change. |
| **Defensive Over-Checking** | Adding paranoid null/boundary checks on trusted internal invariants. | Validate boundaries at the edge (wire deserialization, user input); trust internal type system invariants. |

---

## 2. Lean Code Architecture

### 1. Less Code is Better Code
- Every line of code added is a line that must be compiled, tested, and maintained.
- Prefer standard library functionality before reaching for any third-party crate, dependency, or helper library.
- If a problem can be solved in 10 clean, readable lines, never write 50 lines.

### 2. Dead Code Elimination
- When replacing or refactoring an algorithm or pipeline step, delete the obsolete code immediately.
- Never leave commented-out legacy code, inactive debug prints, or orphan helper functions.

### 3. Flat & Direct Control Flow
- Prefer early returns (`guard` clauses, `if let Err(...) = ... return`) over deeply nested `if/else` ladders.
- Avoid deep inheritance hierarchies or excessive layer-hopping (e.g., Service -> Manager -> Handler -> Processor -> Impl). Keep data flow direct and traceable.

### 4. Zero-Dependency Discipline
- **Rust (`desktop/`)**: Standard library + bounded channels (`crossbeam_channel`). No async runtimes (Tokio is banned outside Linux `bt.rs`).
- **Android (`android/`)**: Pure Jetpack Compose + Oboe C++. Blacklist: Retrofit, Hilt/Dagger, Room, Firebase, external analytics SDKs.
- **Automation (`scripts/`)**: Zero-dependency standard library Python 3 only.
- **Web (`website/`)**: React 19 + Vite + TypeScript. No heavy UI framework runtimes.

---

## 3. Concrete Examples

### ❌ Slop Example (Verbose, Chatty, Speculative)
```kotlin
// Abstract factory interface for creating audio processors
interface AudioProcessorFactory {
    fun createProcessor(): AudioProcessor
}

// Concrete implementation of audio processor
class StandardAudioProcessor : AudioProcessor {
    // Process audio samples from the buffer
    override fun process(buffer: ShortArray): ShortArray {
        // Create a new array to hold processed samples
        val output = ShortArray(buffer.size)
        // Loop over all samples in the buffer
        for (i in buffer.indices) {
            // Copy sample to output array
            output[i] = buffer[i]
        }
        // Return the processed buffer
        return output
    }
}
```

### ✅ Lean Owlmic Standard (Direct, Zero-Alloc, High-Performance)
```kotlin
fun processAudio(samples: ShortArray, count: Int) {
    // Direct in-place mutation or pass-through; zero heap allocation on audio thread
    for (i in 0 until count) {
        samples[i] = samples[i]
    }
}
```

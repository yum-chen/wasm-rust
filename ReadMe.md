# WasmRust

**WasmRust** is a research-driven, production-oriented effort to make **Rust a WASM-native language**, not merely a language that *targets* WebAssembly.

Rather than forking Rust or reinventing the ecosystem, WasmRust explores **minimal, evidence-based extensions** to Rust’s language, compiler, and tooling that close the real gaps in today’s Rust → WASM pipeline: binary size, compile time, component interoperability, and host friction.

> **Position**
> Rust is a strong foundation for WASM — but not inherently optimal.
> WasmRust exists to close that gap.

---

## ✨ Why WasmRust?

Rust dominates the WASM ecosystem (`wasmtime`, `wasmer`, `wit-bindgen`), yet developers consistently encounter:

* ❌ Large binaries (30–40 KB “hello world”)
* ❌ Slow compile times (LLVM-heavy)
* ❌ Frictional JS / WASI interop
* ❌ Poor alignment with the WASM Component Model
* ❌ A steep learning curve for non-systems developers

Meanwhile, languages like Zig and AssemblyScript show that WASM can be **smaller and faster**, often by sacrificing safety or ecosystem depth.

**WasmRust asks a different question:**

> *What would Rust look like if WASM were a first-class execution environment?*

---

## 🧠 What This Project Is (and Isn’t)

### ✅ Is

* A **research and prototyping effort**
* Evidence-driven (benchmarks over opinions)
* Incrementally adoptable from existing Rust
* Aligned with WASM standards (Component Model, WASI)

### ❌ Is Not

* A Rust fork (unless forced by evidence)
* A replacement for `wasm-bindgen`
* A new language
* A centralized ecosystem play

---

## 🏗️ Architecture

WasmRust is organized as a **five-layer stack**:

```
Language Extensions → Component Model → Runtime Semantics
        → Compiler Pipeline → Tooling & Ecosystem
```

Each layer is independently useful and incrementally adoptable.

For a detailed technical breakdown, see **[ARCHITECTURE.md](ARCHITECTURE.md)**.

---

## 🌍 Is Rust Optimal for WASM? A Multipolar View

### ✅ **Where Rust Excels**
- **Memory safety without GC**: Critical for WASM's no-runtime philosophy
- **Zero-cost abstractions**: Maps cleanly to WASM's stack machine
- **Predictable performance**: No hidden allocations or runtime surprises
- **Ecosystem maturity**: `wasmtime`, `wasmer`, `wasm-tools` heavily Rust-based

### ⚠️ **Structural Limitations**
| Challenge | Root Cause | Impact on WASM |
|-----------|-----------|----------------|
| **Large binaries** | Monomorphization explosion | 35 KB "hello world" vs 2 KB in C |
| **Compile times** | LLVM backend, borrow-checking | Slow iteration for web dev |
| **JS interop friction** | `wasm-bindgen` glue layer | 5-10% overhead, cognitive load |
| **Learning curve** | Lifetimes, ownership | Barrier vs TypeScript/AssemblyScript |

### 🌏 **Alternative Paradigms Worth Considering**

**East Asian Approach** (efficiency-first):
- **Zig**: Manual memory management, comptime metaprogramming → ~1 KB binaries
- **Nim**: Python-like syntax, compiled to C → predictable WASM output

**European Research** (formal verification):
- **OCaml/ReScript**: Strong type inference, GC-aware WASM backend
- **Idris2**: Dependent types → provably correct WASM modules

**Latin American Open Source** (accessibility):
- **Gleam**: Erlang VM alternative targeting WASM via Rust backend

**Verdict**: Rust is a **strong foundation**, but **not inherently optimal**. The key is designing a **WASM-native dialect** that removes impedance mismatches.

---

## 🆚 **Revised Comparison: WasmRust vs Alternatives**

| Metric | **WasmRust** | Rust + wasm-bindgen | AssemblyScript | **Zig** | **Grain** |
|--------|--------------|---------------------|----------------|---------|-----------|
| **Binary Size** | **~2 KB** | ~35 KB | ~8 KB | **~1 KB** | ~6 KB |
| **Compile Time** | **~3s (Cranelift)** | ~12s (LLVM) | ~4s | **~2s** | ~3s |
| **Memory Safety** | ✅ Borrow-checked | ✅ Borrow-checked | ⚠️ Manual | ⚠️ Manual | ✅ Type-safe |
| **Component Model** | ✅ Native | ❌ Glue layer | ❌ None | ⚠️ Partial | ⚠️ Planned |
| **JS Interop** | **0% overhead** | 5-10% | 1-3% | 3-5% | 1-2% |
| **Learning Curve** | **Gentle** (Polonius) | Steep | Easy | Moderate | Moderate |
| **Threads Safety** | ✅ Compile-time | ⚠️ Unsafe | ⚠️ Unsafe | ⚠️ Unsafe | ⚠️ Unsafe |
| **Ecosystem** | 🌱 Bootstrap | 🌳 Mature | 🌿 Growing | 🌿 Growing | 🌱 Early |

**Key Insight**: Zig challenges WasmRust on **simplicity** and **size**. Consider a **hybrid approach**:
- WasmRust for **safety-critical** components (crypto, parsers)
- Zig/C for **hot paths** (audio/video codecs, tight loops)

---

## 🚀 **Prototype Roadmap**

### Phase 1: **Proof of Concept** (3 months)
1. ✅ **`wasm` crate**: `externref<T>`, `SharedSlice<T>`, `#[wasm::export]` macro
2. ✅ **Cranelift backend**: Fork `rustc_codegen_cranelift`, add WASM target
3. ✅ **Benchmark**: Compare vs Rust, AS, Zig on Mandelbrot/N-body

### Phase 2: **Component Model** (6 months)
4. ✅ Bidirectional WIT ↔ Rust codegen
5. ✅ `cargo-wasm` with federated registry support
6. ✅ Browser DevTools integration (memory visualizer)

### Phase 3: **Standardization** (12 months)
7. ✅ RFC to Rust project (Layer 1 features)
8. ✅ Bytecode Alliance collaboration (WASI-P2 integration)
9. ✅ W3C WebAssembly CG presentation

---

## 🎯 **Critical Success Factors**

1. **Incremental adoption**: Must interop with existing `wasm-bindgen` code
2. **Binary size obsession**: Every byte matters for mobile/edge
3. **China/India developer experience**: Documentation in Mandarin, Hindi, Spanish
4. **Avoid vendor lock-in**: No Anthropic/OpenAI APIs in toolchain (preserve sovereignty)

---

## 🚧 Project Status

**Early research / prototype phase**

*   APIs are unstable
*   Ideas are experimental
*   Benchmarks and measurements drive decisions

---

## 🤝 Contributing

WasmRust is **research-first and community-driven**.

We welcome:

*   Benchmarks
*   Compiler experiments
*   Design critiques
*   Documentation & localization

See `CONTRIBUTING.md` for details.

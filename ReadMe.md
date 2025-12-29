# WasmRust — Rust-to-WebAssembly Compiler

**WasmRust** is a research-driven, production-oriented Rust-to-WebAssembly compilation system. It aims to make **Rust truly WASM-native**, not merely a language that targets WebAssembly.

> **WasmRust does not ask Rust to become a GC language.**
> **It allows Rust to *target* a GC machine correctly.**

That distinction is the entire project.

WasmRust extends Rust through **minimal, evidence-based compiler and library enhancements**, closing gaps in binary size, compilation speed, component interoperability, and host friction — all while preserving Rust’s safety guarantees.

> **Core Principle:**
> WasmRust = rustc + WASM specialization, not a new language.

---

## ✨ Motivation

Despite Rust’s dominance in the WASM ecosystem (`wasmtime`, `wasmer`, `wit-bindgen`), developers face:

*   **Large binaries** – even simple programs can be 35 KB; alternative WASM-first languages can be smaller.
*   **Slow compilation** – LLVM backend + borrow checking slows iteration.
*   **JS interop friction** – glue layers add overhead and complexity.
*   **Steep learning curve** – ownership and lifetimes can be barriers.
*   **WASM Component Model misalignment** – Rust semantics do not always map cleanly to WASM interfaces.

WasmRust asks:

> *What would Rust look like if WASM were a first-class execution model?*

---

## What Is WasmRust?

WasmRust is a **specialized Rust toolchain** that keeps the Rust frontend unchanged (parser, HIR, MIR, borrow checker) and augments code generation for WASM, providing library-level primitives that map directly to WASM concepts.

```text
┌─────────────────────────────────────────────┐
│                 rustc frontend              │
│   (parsing, HIR, MIR, borrow checking)       │
│                 UNCHANGED                   │
└───────────────────┬─────────────────────────┘
                    ▼
┌─────────────────────────────────────────────┐
│           WASM-specialized codegen           │
│   ┌────────────────┬────────────────────┐   │
│   │ Cranelift WASM │ LLVM WASM           │   │
│   │ (dev builds)   │ (release builds)    │   │
│   └────────────────┴────────────────────┘   │
└─────────────────────────────────────────────┘
                    ▼
┌─────────────────────────────────────────────┐
│         crates/wasm (zero-cost APIs)         │
│   externref, threads, components, memory    │
└─────────────────────────────────────────────┘
```
---

## 🌍 Design Philosophy

1.  **WASM-native semantics**: Model WebAssembly concepts (memory, resources, components) directly.
2.  **Safety without bloat**: Retain Rust’s memory safety while avoiding unnecessary runtime overhead.
3.  **Incremental adoption**: Interoperate with existing Rust, `wasm-bindgen`, and WASI code.
4.  **Global and federated**: Avoid centralized registries and vendor lock-in.
5.  **Evidence-driven**: Features are justified through benchmarks, size, or correctness.

---

## 🏗 Architecture Overview

WasmRust is structured as a **five-layer stack**:

```
┌─────────────────────────────┐
│ Layer 5 — Tooling & Ecosystem│
│ Registries, debugging, profiler│
├─────────────────────────────┤
│ Layer 4 — Compiler           │
│ WasmIR, Cranelift, LLVM, PGO│
├─────────────────────────────┤
│ Layer 3 — Runtime Semantics  │
│ Multi-memory, regions, threads│
├─────────────────────────────┤
│ Layer 2 — Component Model    │
│ WIT-native imports/exports   │
├─────────────────────────────┤
│ Layer 1 — Core Language      │
│ Linear types, effects, concurrency│
└─────────────────────────────┘
```

---

### The 5 Layers in Detail

#### Layer 1 — Core Language Extensions & `crates/wasm`

The foundation of WasmRust is the `crates/wasm` library. It is `no_std`, dependency-free, runtime-free, and compiler-agnostic. Most WASM semantics belong at the library boundary, providing the compiler with semantic hooks for optimization and allowing stable Rust users to adopt WASM-first APIs today.

This approach is deliberate:
*   It allows for the explicit modeling of WebAssembly concepts, such as `externref`, shared memory, and components.
*   It provides a semantic hook for the WasmRust compiler to perform optimizations.
*   This approach mirrors the evolution of `core`, `alloc`, and `std` as a set of layered abstractions.

*   **Linear Types**: Enforce use-once semantics for WASM resources to prevent leaks.
  ```rust
  #[wasm::linear]
  struct CanvasContext(wasm::Handle);

  impl CanvasContext {
      fn draw(&mut self) { /* ... */ }
      // This consuming method moves ownership, preventing further use.
      fn into_bitmap(self) -> ImageData { /* ... */ }
  }
  ```
*   **Structured Concurrency**: Scoped threads with automatic joining and lifetime-bound safety.
  ```rust
  use wasm::thread::scope;

  #[wasm::export]
  fn parallel_transform(data: SharedSlice<f32>) -> Result<(), Error> {
      scope(|s| {
          for chunk in data.chunks(1000) {
              s.spawn(|| process(chunk)); // Lifetime tied to scope
          }
          // All threads are automatically joined here
      })?;
      Ok(())
  }
  ```
*   **Effect System**: Track side effects like JS calls or I/O at the type level to enable optimizations like dead-effect elimination.
  ```rust
  #[wasm::effect(js_call, atomic_read)]
  fn fetch_and_cache(url: &str) -> Result<Vec<u8>, Error> {
      let data = js::fetch(url)?;
      CACHE.store(url, data);
      Ok(data)
  }
  ```

#### Layer 2 — Component Model

Treats WIT as a first-class interface, enabling type-safe, bidirectional Rust ↔ WASM code generation without glue code.

```rust
#[wasm::wit]
interface crypto {
    resource key-pair {
        constructor(algorithm: string);
        sign: func(data: bytes) -> bytes;
    }
}
```

#### Layer 3 — Runtime Semantics

*   **Multi-region memory**: First-class support for data residency and isolation.
*   **Streaming compilation hints**: Optimize binary layout for faster Time-to-Interactive in browsers.

#### Layer 4 — Compiler Strategy

*   **Cranelift-first dev builds** for fast iteration (~2s for 10k LOC and 5-10x faster than LLVM).
*   **LLVM release builds** for aggressive optimizations, `wasm-opt`, and Profile-Guided Optimization.
*   **WasmIR**: A stable intermediate representation that captures linearity, reference types, and ownership invariants.

#### Layer 5 — Tooling & Ecosystem

*   **Federated registries** to avoid centralized lock-in and geopolitical restrictions.
*   **WASM-aware debugging tools** for memory visualization and inspection.

---

## Core Features

| Feature           | Description                                              |
| ----------------- | -------------------------------------------------------- |
| WASM-native types | `ExternRef`, `FuncRef`, `SharedSlice`, `Pod`             |
| Linear types      | Enforce move-only semantics for WASM resources           |
| Component Model   | Compiler-verified ABI, WIT bindings                      |
| JS Interop        | Zero-copy, predictable boundary cost                     |
| Threading         | Scoped concurrency, fallback in unsupported environments |

---

## Compilation Pipeline

```mermaid
graph LR
    A[Rust Source] --> B[HIR/MIR]
    B --> C[WasmIR]
    C --> D{Build Profile}
    D -->|Dev| E[Cranelift]
    D -->|Release| F[LLVM]
    E --> G[Fast WASM]
    F --> H[Optimized WASM]
    H --> I[wasm-opt]
    I --> J[Component Wrapper]
```
**WasmIR** is a stable boundary that encodes:
*   Linear memory operations with bounds checking.
*   Reference types (`externref`, `funcref`) with lifetime tracking.
*   Component Model calling conventions.
*   Ownership and linearity invariants.
---
## Repository Structure

```
wasmrust/
├── compiler/                # rustc extensions & backends
│   ├── codegen-cranelift/   # WASM-tuned Cranelift backend
│   └── codegen-llvm/        # WASM-optimized LLVM backend
│
├── crates/
│   ├── wasm/                # Core zero-cost WASM abstractions
│   └── wasm-macros/         # Proc macros for Component Model / WIT [planned]
│
├── tooling/
│   └── cargo-wasm/          # WASM-aware Cargo frontend [planned]
│
├── docs/
│   ├── PRD-WasmRust.md      # WasmRust Prouct Requirements Document
│   ├── TSD-WasmRust.md      # WasmRust Technical Specification Document
│   ├── SAFETY.md            # Unsafe invariants per type / crate
│   ├── compiler-contract.md # Formal compiler ↔ crate contracts
│   └── architecture/
│
└── ReadMe.md
```
---

## Incremental Adoption

#### What Works Without WasmRust?
Everything in `crates/wasm`: it compiles on **stable Rust**, produces valid WASM, and has no dependency on a custom compiler. WasmRust **enhances**, but does not gate, functionality.

#### What Requires the WasmRust Compiler?
Native Component Model emission, Cranelift-accelerated builds, and advanced optimizations like PGO and WASM-aware thin monomorphization. These cannot be achieved from a library alone.

---

## Contracts & Governance

*   **Language Surface Contract**: Core (80%): Standard Rust; Extensions (15%): `wasm` crate; Plugins (4%): `-Z` flags; Hard Fork (<1%): Minimal changes if required.
*   **Compiler ↔ Crate Contract**: The compiler assumes invariants for types like `ExternRef` and `SharedSlice`. Unsafe operations must maintain these invariants, which are documented in `SAFETY.md` and checked by compiler passes and lints (`wasm-recognition`).
    *   `ExternRef<T>` and `FuncRef` are opaque handles with valid lifetime markers.
    *   `SharedSlice<T>` contains only `Pod` types; aliasing and bounds are enforced.
    *   Linear types (`#[wasm::linear]`) follow move semantics; the compiler assumes no implicit copies.
    *   Component imports/exports use WIT-derived types; the ABI must match exactly.
*   **Notes on SAFETY.md**: Contains **formal unsafe invariants** per type. It serves as authoritative documentation for both crate users and compiler developers.
*   **Governance & Direction**: Upstream-friendly, library-first stabilization, and RFC-driven evolution.

---

## Host Profile Support

| Host Profile | Threading                     | JS Interop      | Component Model | Memory Regions |
| ------------ | ----------------------------- | --------------- | --------------- | -------------- |
| Browser      | SharedArrayBuffer + COOP/COEP | Direct calls    | Partial         | No             |
| Node.js      | Worker threads                | Native bindings | Polyfill        | No             |
| Wasmtime     | wasi-threads                  | Host functions  | Full            | Configurable   |
| Embedded     | No                            | No              | Partial         | No             |
---

## Testing and Verification

*   Property-Based Testing: binary size, monomorphization, ownership enforcement, threading safety.
*   Cross-Language ABI Testing: Zig, C, and other WASM components.
*   Reproducible Builds and Performance Benchmarks.

---

## Comparative Snapshot

| Metric          | WasmRust    | Rust+bindgen | Zig   | AssemblyScript |
| --------------- | ----------- | ------------ | ----- | -------------- |
| Binary size     | ~2 KB       | ~35 KB       | ~1 KB | ~8 KB          |
| Compile time    | ~3s         | ~12s         | ~2s   | ~4s            |
| Memory safety   | ✅           | ✅            | ⚠️    | ⚠️             |
| Component Model | ✅           | ❌            | ⚠️    | ❌              |
| Thread Safety   | ✅           | ⚠️ Unsafe    | ⚠️    | ⚠️             |

---

## 🚀 Roadmap

**Phase 1 — Proof of Concept (3 months)**
* `wasm` crate: core WASM abstractions
* Cranelift backend for dev builds
* Benchmark comparisons

**Phase 2 — Component Model (6 months)**
* WIT ↔ Rust bidirectional codegen
* `cargo-wasm` with federated registry
* DevTools memory visualization

**Phase 3 — Standardization (12 months)**
* RFCs for Layer 1 features
* Collaboration with Bytecode Alliance
* W3C WebAssembly CG presentation

---

## Non-Goals

* A Rust fork or new language
* Replacing `wasm-bindgen` initially
* A JavaScript framework or runtime

---

## Where to Start

*   📦 Use `crates/wasm` for low-level WASM code today.
*   📖 Read tech specs in [`docs/TSD-WasmRust.md`](docs/TSD-WasmRust.md).
*   🧪 Experiment with Cranelift WASM builds (nightly).
*   🛠️ Contribute to core abstractions before compiler work.

# WasmRust Architecture 2.0: Design Principles

## Introduction

WasmRust's Architecture 2.0 is a deliberate evolution of the project's vision, designed to solidify its position as the premier toolchain for building systems-grade WebAssembly components with Rust. The following principles guide the design, development, and decision-making processes for this architecture.

---

### 1. **Rust First, Not a Rust Fork**

The primary directive of WasmRust is to **enhance and specialize Rust for WebAssembly, not to create a new language**.

- **Compatibility is Key**: WasmRust must remain a superset of standard Rust. All valid Rust code should compile and run correctly with the WasmRust toolchain.
- **Ecosystem is Sacred**: Full, zero-cost access to the `crates.io` ecosystem is non-negotiable. Architectural decisions must not create a walled garden.
- **Upstream Alignment**: We favor solutions that have a clear path to eventual upstreaming into `rustc` and related projects. We avoid features that are fundamentally incompatible with Rust's long-term direction.

---

### 2. **Dual-Mode Memory Management**

WasmRust embraces the idea that developers need the right tool for the right job, especially concerning memory management. Architecture 2.0 formalizes a **dual-mode compilation system**.

- **Ownership as the Default**: Rust's ownership and borrowing system remains the default, providing the foundation for performance, safety, and control.
- **GC as a First-Class Option**: WasmGC is not an afterthought. The `#[wasm::gc]` attribute enables a first-class, garbage-collected programming model for parts of an application where development velocity and simplicity are paramount.
- **Clear and Explicit Boundaries**: The interface between ownership-based and GC-based code must be explicit and safe. The compiler will enforce these boundaries to prevent memory model violations.

---

### 3. **Library-First Semantics**

Core WebAssembly semantics should be expressed in a library (`crates/wasm`) before they are accelerated by the compiler.

- **Meaning Resides in the Crate**: The `wasm` crate is the source of truth for the semantics of types like `ExternRef` and `SharedSlice`. It must be usable on stable Rust without the WasmRust compiler.
- **The Compiler Accelerates, It Does Not Invent**: The role of the WasmRust compiler is to recognize the semantic patterns established in the `wasm` crate and replace them with more efficient, lower-level implementations (e.g., native WASM instructions).
- **Enables Incremental Adoption**: This principle allows developers to adopt WasmRust's patterns and abstractions immediately, with the option to add the compiler later for enhanced performance and features.

---

### 4. **The Formal Compiler-Crate Contract**

All compiler optimizations that rely on the semantics of the `wasm` crate must be governed by a formal, machine-verifiable contract.

- **No "Magic"**: Optimizations are not based on ad-hoc pattern matching of unstable internal representations. They are based on documented and versioned invariants.
- **Soundness through Verification**: A dedicated MIR pass (`verify_wasm_invariants`) and a lint group (`wasm-recognition`) are responsible for ensuring that all optimizations adhere to the contract. This prevents unsoundness and provides stability.
- **Preserves the Escape Hatch**: The contract ensures that if the WasmRust compiler were removed, the code would still compile and run correctly on stable `rustc`, albeit without the performance benefits.

---

### 5. **Performance Parity with WASM-Native Languages**

WasmRust aims to eliminate the trade-offs between using a mature language like Rust and achieving the performance characteristics of newer, WASM-first languages.

- **Binary Size is a Feature**: A "hello, world" with WasmGC should be < 2KB, competitive with MoonBit and Zig.
- **Fast Iteration is Mandatory**: Development builds, powered by Cranelift, must provide compilation speeds that are an order of magnitude faster than traditional LLVM-based builds (~2s for 10k LOC).
- **Release Performance is Uncompromised**: Release builds will continue to use LLVM and benefit from advanced optimization techniques like Profile-Guided Optimization (PGO) and `wasm-opt`.

---

### 6. **Principled Tooling and Ecosystem Integration**

WasmRust is more than a compiler; it is a complete toolchain that must feel like a natural extension of the Rust ecosystem.

- **`cargo-wasm` is the Entry Point**: All functionality should be exposed through a `cargo-wasm` subcommand that is intuitive for existing Rust developers.
- **IDE Support is Crucial**: Features must be designed with `rust-analyzer` and other IDE tools in mind to ensure that auto-completion, type checking, and debugging work seamlessly.
- **Decentralization and Federation**: The tooling and registry infrastructure should be designed to avoid central points of failure and vendor lock-in, supporting a federated model for crate distribution.

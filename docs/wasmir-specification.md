# WasmIR Specification (Version 1.0)

## 1. Overview

WasmIR (WasmRust Intermediate Representation) is the stable, explicit semantic boundary between the Rust MIR and the WasmRust compiler backends. Its design is governed by the principle that every semantic transformation must be either proven equivalent to Rust MIR or explicitly declared as a new contract boundary.

This document provides the formal specification for WasmIR. Any undocumented behavior is considered a bug.

## 2. Design Goals

*   **Semantic Stability**: To provide a durable contract between the Rust frontend and Wasm-specific backends.
*   **Invariant Enforcement**: To make Rust's ownership, aliasing, and lifetime invariants explicit and machine-readable.
*   **Performance Modeling**: To enable sound, verifiable, and predictable WASM-specific optimizations.
*   **WasmGC Readiness**: To be forward-compatible with the WebAssembly Garbage Collection (WasmGC) extension by design.

## 3. Type System

WasmIR's type system is designed to be sound and expressive, capturing both the semantics of Rust and the capabilities of WebAssembly.

### 3.1. Value Types

Value types represent raw data that can be manipulated directly by WASM instructions.

*   `i32`: 32-bit integer
*   `i64`: 64-bit integer
*   `f32`: 32-bit floating-point
*   `f64`: 64-bit floating-point

### 3.2. Reference Types

Reference types represent handles to resources or objects that are not directly mapped in linear memory.

*   `externref`: An opaque reference to a host-provided object (e.g., a JavaScript object). It is non-nullable.
*   `funcref`: A reference to a function. It is non-nullable.
*   `anyref`: A nullable reference that can hold `externref`, `funcref`, or other reference types.

### 3.3. Memory Model

WasmIR defines two memory spaces:

*   **Linear Memory**: A single, contiguous, and growable array of bytes, corresponding to WebAssembly's linear memory. Pointers into this memory are represented as `i32` or `i64`.
*   **Reference Space**: A conceptual space where reference-typed values exist. These values are not directly addressable from linear memory.

## 4. Instruction Set

Each WasmIR instruction is defined with its syntax, operands, and a formal description of its semantics.

### 4.1. Local Variable Instructions

*   **Syntax**: `local.get <local_index>`
    *   **Semantics**: Pushes the value of the local variable at `<local_index>` onto the value stack.
*   **Syntax**: `local.set <local_index> <value>`
    *   **Semantics**: Pops `<value>` from the value stack and stores it in the local variable at `<local_index>`.

### 4.2. Control Flow Instructions

*   **Syntax**: `br <block_id>`
    *   **Semantics**: Unconditionally transfers control to the basic block identified by `<block_id>`.
*   **Syntax**: `br_if <block_id> <condition>`
    *   **Semantics**: Pops `<condition>` (an `i32`) from the value stack. If `<condition>` is non-zero, transfers control to `<block_id>`. Otherwise, execution continues to the next instruction.

### 4.3. Ownership and Invariant Instructions

These instructions make Rust's ownership and borrowing system explicit.

*   **Syntax**: `linear.consume <value>`
    *   **Semantics**: Consumes `<value>`, which must be of a linear type. This instruction marks the end of the value's lifetime. Any subsequent use of the same value is a validation error. This corresponds to a move in Rust.
*   **Syntax**: `invariant.check.aliasing <ptr1> <ptr2>`
    *   **Semantics**: Asserts that the memory ranges pointed to by `<ptr1>` and `<ptr2>` do not overlap. This makes the `noalias` invariant from Rust explicit for the optimizer.

## 5. Contract Alignment

WasmIR is designed to be a direct representation of the compiler ↔ crate contracts defined in `SAFETY.md`.

*   **`SharedSlice<T>`**: A `SharedSlice` is lowered to a WasmIR struct containing an `i32` (pointer) and an `i32` (length). The invariant that `T` must be `Pod` is checked during the MIR-to-WasmIR lowering.
*   **`ExternRef<T>`**: An `ExternRef` is lowered to the `externref` type in WasmIR. The compiler is responsible for ensuring that the handle is used in a way that is consistent with its lifetime.
*   **Linear Types (`#[wasm::linear]`)**: A struct or resource marked `#[wasm::linear]` is treated as a linear type in WasmIR. The `linear.consume` instruction is emitted when the value is moved or goes out of scope, enforcing use-once semantics.

## 6. Non-Goals

WasmIR is *not*:

*   A generic IR for arbitrary languages. It is specifically designed for Rust-to-WASM compilation.
*   A representation of Rust's lifetime syntax. Lifetimes are checked and erased in the `rustc` frontend; WasmIR only deals with the resulting ownership and aliasing invariants.
*   A stable format for long-term code distribution. It is an internal compiler representation, and its format may change between compiler versions.

## 7. Examples

The following are illustrative examples. The formal, testable mapping from Rust MIR to WasmIR is defined in the executable specification tests in `tests/wasmir_spec_snapshots.rs`.

### Example: Simple `add` function

**Rust:**
```rust
fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**Conceptual WasmIR:**
```
function "add"(param i32, param i32) -> i32 {
  bb0:
    v0 = local.get 0
    v1 = local.get 1
    v2 = i32.add v0, v1
    return v2
}
```

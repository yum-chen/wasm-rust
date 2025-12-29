# WasmIR Specification (Version 1.1)

## 1. Overview and Principles

WasmIR (WasmRust Intermediate Representation) is the stable, explicit semantic boundary between Rust's Mid-level Intermediate Representation (MIR) and the WasmRust compiler backends. Its design is governed by the principle that **every semantic transformation must be either proven equivalent to Rust MIR or explicitly declared as a new contract boundary.**

This document provides the formal specification for WasmIR. Any behavior not documented here is a bug.

## 2. Type System and Memory Model

### 2.1. Value and Reference Types
WasmIR uses WebAssembly's core value types (`i32`, `i64`, `f32`, `f64`) and reference types (`externref`, `funcref`, `anyref`).

### 2.2. Linear Types and State Transitions
A type marked `#[wasm::linear]` is subject to a strict "exactly once" usage semantic. This is enforced by the MIR linear analysis passes and represented in WasmIR through explicit state annotations.

The liveness of a linear type is tracked through the following states:

*   **`Uninitialized`**: The variable has been declared but not yet assigned a value.
*   **`Active`**: The variable holds a valid, live linear resource. It *must* be consumed on every subsequent control flow path.
*   **`Consumed`**: The variable's resource has been successfully moved or destructured. Any further use is a compile-time error.

These states are not types themselves but are metadata attached to WasmIR locals that the backend uses to validate `linear.consume` instructions.

## 3. Instruction Set

### 3.1. Ownership and Invariant Instructions

These instructions make Rust's ownership and borrowing system explicit and verifiable.

*   **Syntax**: `linear.consume <local>`
    *   **Semantics**: Consumes the linear resource held in `<local>`. This instruction is emitted by the `LinearPathCompleteness` pass. It is a validation error if the state of `<local>` is not `Active`. After this instruction, the state of `<local>` transitions to `Consumed`.
    *   **Maps From MIR**: `mir::StatementKind::Assign` where the Rvalue is a `mir::Operand::Move` of a linear type.

*   **Syntax**: `invariant.check.aliasing <ptr1> <ptr2>`
    *   **Semantics**: Asserts that the memory ranges pointed to by `<ptr1>` and `<ptr2>` do not overlap. This makes the `noalias` invariant from Rust explicit for the optimizer.
    *   **Maps From MIR**: Inferred from MIR operations on `&mut T` references that are known not to alias.

*   **Syntax**: `invariant.check.drop <local>`
    *   **Semantics**: Asserts that the linear resource in `<local>` has been consumed. This is inserted by the `LinearObjectDropScan` pass before any implicit `Drop` terminator for a linear type, effectively turning the implicit drop into a compile error.
    *   **Maps From MIR**: `mir::TerminatorKind::Drop` for a linear type.

## 4. WebAssembly Component Model Metadata

To satisfy the requirements of the WebAssembly Component Model, a WasmIR function must be annotated with the following metadata:

*   **Interface Information**: The WIT interface this function belongs to, including the package, interface, and function name.
*   **Resource Mappings**: For any `externref` or linear type that represents a Component Model resource, WasmIR must track the WIT resource type it maps to.
*   **ABI Variant**: An annotation specifying whether the function uses the `canon lift` or `canon lower` ABI conventions for component function calls.

This metadata is attached to the top-level `WasmIR` struct and is used by the final WasmIR-to-WASM codegen pass to generate the correct custom sections in the final `.wasm` file.

## 5. MIR to WasmIR Traceability Matrix

This matrix provides a mapping of key Rust MIR constructs to their WasmIR equivalents. Any MIR instruction not on this list is a gap in the current specification.

| Rust MIR Construct | WasmIR Equivalent(s) | Notes |
| --- | --- | --- |
| `StatementKind::Assign` (Copy) | `local.set`, `i32.add`, etc. | Standard assignment is lowered to the corresponding WasmIR operation. |
| `StatementKind::Assign` (Move of linear type) | `linear.consume` | The core of the linear type system. |
| `StatementKind::StorageDead` (on linear type) | `invariant.check.drop` | Enforces that linear types cannot be implicitly dropped. |
| `TerminatorKind::Drop` (on linear type) | `invariant.check.drop` | Same as `StorageDead`. |
| `TerminatorKind::Return` | `return` | Direct mapping. |
| `TerminatorKind::Goto` | `br` | Direct mapping. |
| `TerminatorKind::SwitchInt` | `br_table` or sequence of `br_if` | Direct mapping. |
| `Rvalue::BinaryOp` | `i32.add`, `i32.sub`, etc. | Direct mapping. |
| `Rvalue::Ref` (`&mut T`) | `(value)` + `invariant.check.aliasing` | The pointer itself is a value, but its usage implies aliasing invariants. |

## 6. Non-Goals

WasmIR is *not*:

*   A generic IR for arbitrary languages.
*   A representation of Rust's lifetime syntax. Lifetimes are checked and erased by `rustc`; WasmIR only enforces the resulting ownership invariants.
*   A stable format for long-term code distribution. It is an internal compiler representation.

# WasmRust Task 3 Validation Report

## 1. Verdict per Subtask

*   **3.1 Fork rustc_codegen_cranelift and integrate into WasmRust:** ❌ **FAIL**
*   **3.2 Design and document WasmIR specification:** ❌ **FAIL**
*   **3.3 Implement MIR → WasmIR lowering pass:** ❌ **FAIL**
*   **3.4 Implement WasmIR → WASM codegen:** ❌ **FAIL**

## 2. Unmet Invariants or Undocumented Assumptions

*   **Subtask 3.1:** The core invariant that the Cranelift backend is a proper, version-pinned fork of `rustc_codegen_cranelift` is unmet. The implementation is a partial, custom backend that does not appear to be based on the official `rustc_codegen_cranelift`. There is no evidence of MIR parity testing, meaning there is no guarantee that Rust semantics are preserved.
*   **Subtask 3.2:** The WasmIR specification is incomplete and lacks formal definitions for ownership, aliasing, and the memory model. It does not explicitly reference the compiler ↔ crate contracts, and there are no executable, test-checked examples to prevent ambiguity.
*   **Subtask 3.3:** The MIR to WasmIR lowering pass is highly incomplete, with numerous "fallback" and "simplified" cases. There is no evidence that Rust's ownership, borrowing, and lifetime semantics are preserved. The lowering pass does not make critical invariants (like linearity) explicit in the generated WasmIR.
*   **Subtask 3.4:** The WasmIR to WASM codegen is also incomplete, with placeholder implementations for all required optimizations. There are no tests to prove the soundness of the implemented instruction mappings or to validate the binary layout.

## 3. Semantic Divergence Risks

*   **High:** The lack of MIR parity testing in subtask 3.1 means there is a high risk of accidental divergence from Rust's semantics. The custom implementation of the backend could easily misinterpret or ignore subtle aspects of MIR.
*   **High:** The incomplete and informal nature of the WasmIR specification (subtask 3.2) creates a high risk of semantic ambiguity. Different parts of the compiler could interpret the same WasmIR construct differently, leading to subtle bugs.
*   **Critical:** The incomplete MIR to WasmIR lowering pass (subtask 3.3) presents a critical risk of semantic divergence. By failing to correctly lower all MIR constructs, the implementation could silently miscompile valid Rust code, leading to undefined behavior at runtime.
*   **High:** The incomplete WasmIR to WASM codegen (subtask 3.4) also presents a high risk of semantic divergence. Without proper tests and soundness proofs, the generated WASM could be incorrect, even if the input WasmIR is valid.

## 4. Performance-Model Readiness

*   The current implementation is **not ready** for performance modeling. The WasmIR specification does not make a clear distinction between GC and linear memory, which is a prerequisite for adding WasmGC support. The optimization passes are all placeholders, so there is no evidence of performance-sensitive design. The lack of a stable WasmIR makes it impossible to reason about the performance implications of different language constructs.

## 5. Concrete Remediation Checklist

1.  **Subtask 3.1:**
    *   [ ] Properly fork the `rustc_codegen_cranelift` repository and integrate it as a submodule or vendored dependency.
    *   [ ] Implement a CI job that performs MIR dump comparison tests between the LLVM and Cranelift backends to ensure semantic parity.
    *   [ ] Add negative compilation tests to ensure that unsupported features fail deterministically.
2.  **Subtask 3.2:**
    *   [ ] Formalize the WasmIR specification, providing detailed semantics for all instructions, types, and the memory model.
    *   [ ] Explicitly document ownership and aliasing semantics in the specification.
    *   [ ] Create a suite of executable snapshot tests that map Rust MIR to expected WasmIR output, covering all instruction classes.
    *   [ ] Add a "Non-Goals" section to the specification to prevent semantic creep.
3.  **Subtask 3.3:**
    *   [ ] Complete the MIR to WasmIR lowering pass, removing all "fallback" and "simplified" cases.
    *   [ ] Implement tests to verify the preservation of Rust's ownership, borrowing, and lifetime semantics.
    *   [ ] Ensure that unsafe invariants are surfaced and made explicit in the generated WasmIR.
    *   [ ] Add support for preserving debug information during the lowering process.
4.  **Subtask 3.4:**
    *   [ ] Implement the required WASM-specific optimizations (thin monomorphization, streaming layout, etc.).
    *   [ ] For each optimization, document the invariant it relies on and prove that the invariant is enforced upstream.
    *   [ ] Implement a comprehensive suite of tests for instruction mapping, optimization soundness, and binary layout.
    .
    *   [ ] Implement differential execution tests that compare the behavior of LLVM-generated and Cranelift-generated WASM.

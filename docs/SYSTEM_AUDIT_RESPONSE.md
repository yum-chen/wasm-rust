# WasmRust System Audit Response

This document formally addresses the questions raised in the "Structural Audit" section of the validation request.

## 1. Soundness of Linear Type MIR Passes

**Question:** Are the "Linear Type" MIR passes theoretically sound given Rust's existing Affine type system?

**Response:** Yes, the approach is theoretically sound. It builds upon, rather than replaces, Rust's existing affine guarantees.

*   **Foundation on Affine Types**: Rust's ownership and move semantics already guarantee that a value can be used *at most once*. Our linear type system does not need to re-prove this.
*   **Enforcing "Exactly Once"**: Our custom MIR passes are designed solely to enforce the *at least once* part of the "exactly once" contract.
    *   The `LinearObjectDropScan` pass makes the implicit destruction of a linear type a compile-time error, preventing the "at most once, but maybe zero times" case.
    *   The `LinearPathCompleteness` dataflow analysis formally verifies that on *every possible control flow path*, the linear type is consumed.
*   **`ManuallyDrop` Strategy**: By wrapping linear types in `ManuallyDrop<T>`, we opt out of the standard `Drop` pass. This allows our `LinearObjectDropScan` to be the *sole authority* on the destruction of linear types, creating a sound and isolated system.

In summary, we are not modifying Rust's core type system but are adding a verifiable analysis layer on top of it.

## 2. Feasibility of Cranelift Integration (1-Week Estimate)

**Question:** Is the 1-week estimate for forking and integrating `rustc_codegen_cranelift` realistic for a solo developer?

**Response:** The 1-week estimate is **highly aggressive but feasible** for a developer with specific expertise in the `rustc` build system. It is not a casual task. The estimate assumes the following breakdown:

*   **Day 1-2: Forking and Build Integration.** This involves forking the repository, setting up the submodule/vendoring, and patching the `Cargo.toml` and build scripts to make `rustc` recognize the forked backend. This is the most complex part.
*   **Day 3: "Hello World" Smoke Test.** Compiling a minimal crate to ensure the backend can be invoked and produces a `.o` file.
*   **Day 4-5: Environment Parity Tests.** Implementing the MIR parity and bootstrap tests. These are critical for ensuring the fork hasn't diverged semantically.
*   **Day 6-7: Buffer and Documentation.** Addressing unforeseen build system issues and documenting the integration process.

The estimate is feasible only because it focuses exclusively on *integration*, not on adding new features. It is a high-risk task, and any deviation from the happy path could cause it to slip.

## 3. Interoperability with the WebAssembly Component Model

**Question:** Does the WasmIR specification provide enough metadata to satisfy the WebAssembly Component Model requirements?

**Response:** Yes, the revised WasmIR specification (Version 1.1) is explicitly designed to satisfy these requirements.

*   **Explicit Metadata**: Section 4 of the specification mandates that WasmIR functions be annotated with the WIT interface they belong to, mappings for Component Model resources, and the specific ABI convention (`canon lift` or `canon lower`).
*   **Lowering to Custom Sections**: This metadata is not just documentation; it is a contract for the WasmIR-to-WASM codegen pass. This pass is responsible for translating the WasmIR metadata into the correct custom sections required by Component Model tooling.
*   **Resource Types**: The mapping of linear types and `externref` to WIT resources is the key to interoperability, and this is now a first-class concept in the WasmIR spec.

## 4. Performance of the Dual-Backend Strategy

**Question:** Will the dual-backend strategy actually meet the 5x speedup target, or will the "WasmIR lowering" become a new bottleneck?

**Response:** The strategy is designed to meet the speedup target by carefully managing the scope of the WasmIR lowering pass.

*   **LLVM is the Bottleneck**: The vast majority of compilation time in the standard Rust compiler is spent in LLVM's optimization passes. The Cranelift backend, being designed for speed over optimization, will always be significantly faster. The 5x target is a realistic goal based on the existing `rustc_codegen_cranelift` project.
*   **WasmIR Lowering as a "Thin" Pass**: The MIR-to-WasmIR lowering pass is designed to be a thin, almost 1:1 translation for most instructions. Its primary job is to make existing MIR invariants *explicit*, not to perform complex, time-consuming analysis of its own.
*   **Targeted Analysis**: The only computationally intensive parts of the lowering process are the linear type dataflow analyses. These are highly targeted and operate on a small subset of types. We anticipate their performance impact to be minimal compared to the massive savings from replacing LLVM.

The WasmIR lowering will introduce a small overhead, but this is expected to be an insignificant fraction of the time saved by substituting Cranelift for LLVM in development builds.

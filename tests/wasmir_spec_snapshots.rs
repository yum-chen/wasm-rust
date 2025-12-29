//! Executable Specification for WasmIR
//!
//! This file contains snapshot tests that serve as the executable specification for
//! the MIR-to-WasmIR lowering pass. Each test defines a piece of Rust code and
//! asserts that the lowered WasmIR matches a stored snapshot.
//!
//! This approach ensures that every lowering rule is explicit, documented, and
//! test-checked against the compiler.

// Note: The testing framework and macros used here are hypothetical and would
// need to be implemented as part of the WasmRust test harness.

#[cfg(test)]
mod tests {
    use wasmrust_test_macros::mir_to_wasmir_snapshot;

    /// Verifies the lowering of a simple arithmetic function.
    #[test]
    #[mir_to_wasmir_snapshot]
    fn test_add_function_lowering() {
        // The Rust source code to be lowered.
        fn add(a: i32, b: i32) -> i32 {
            a + b
        }
    }

    /// Verifies that ownership moves are correctly translated to `linear.consume`.
    #[test]
    #[mir_to_wasmir_snapshot]
    fn test_linear_type_move_lowering() {
        // Assume `LinearResource` is a type marked with `#[wasm::linear]`.
        struct LinearResource;

        fn consume_resource(res: LinearResource) {
            // The move of `res` into this function should result in a
            // `linear.consume` instruction in the caller's WasmIR.
        }
    }

    /// Verifies that pointer aliasing invariants are made explicit.
    #[test]
    #[mir_to_wasmir_snapshot]
    fn test_aliasing_invariant_lowering() {
        // This test ensures that the `noalias` property of the two mutable
        // slices is captured by an `invariant.check.aliasing` instruction.
        fn sum_slices(slice1: &mut [i32], slice2: &mut [i32]) {
            for i in 0..slice1.len() {
                slice1[i] += slice2[i];
            }
        }
    }

    /// Verifies that `ExternRef<T>` is correctly lowered to the `externref` type.
    #[test]
    #[mir_to_wasmir_snapshot]
    fn test_externref_lowering() {
        // Assume `HostObject` is a type that is represented as an `ExternRef`.
        struct HostObject;

        fn get_host_object_id(obj: &HostObject) -> u32 {
            // The `obj` parameter should be lowered to the `externref` type.
            // (Further implementation would be needed to show how methods are called)
            0
        }
    }
}

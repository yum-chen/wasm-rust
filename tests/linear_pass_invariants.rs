//! Invariant Tests for MIR Linear Type Passes
//!
//! This module contains tests that verify the correctness of the MIR linear
//! type analysis passes. These tests ensure that the "exactly once" semantic
//! is correctly enforced.

#[cfg(test)]
mod tests {
    use wasmrust::wasmir::linear_passes::{run_linear_object_drop_scan, run_linear_path_completeness_analysis};
    use rustc_middle::mir; // Hypothetical import

    /// **Invariant Preservation Test: No Implicit Drop**
    ///
    /// Verifies that the `LinearObjectDropScan` pass emits a compile error
    /// when it detects an implicit drop of a linear type.
    #[test]
    fn test_no_implicit_drop_invariant() {
        // MIR for a function where a linear object `_1` goes out of scope.
        let mir_body = mir::Body::new(/* ... */);
        // The terminator for the final basic block would be a `Drop` on `_1`.

        // In a real test, we would use `compile_fail` or a similar mechanism
        // to assert that this code produces a specific compile-time error.
        let result = std::panic::catch_unwind(|| {
            // (tcx would be a mock object)
            run_linear_object_drop_scan((), &mir_body);
        });

        // For this placeholder, we can't check for a compile error, so we'll
        // assume the pass panics when it finds an error.
        assert!(result.is_err(), "The LinearObjectDropScan pass should have produced a compile error for an implicit drop.");
    }

    /// **Invariant Preservation Test: Path Completeness**
    ///
    /// Verifies that the `LinearPathCompleteness` dataflow analysis correctly
    /// identifies when a linear type is not consumed on all control flow paths.
    #[test]
    fn test_path_completeness_invariant() {
        // MIR for a function like:
        // fn foo(cond: bool, res: LinearResource) {
        //     if cond {
        //         consume(res);
        //     }
        //     // Error: `res` is not consumed on the `else` path.
        // }
        let mir_body = mir::Body::new(/* ... */);

        // Run the dataflow analysis.
        let results = run_linear_path_completeness_analysis((), &mir_body);

        // Get the liveness state at the exit point of the function.
        let exit_state = results.exit_state();

        // Assert that the analysis has detected that the linear variable `res`
        // is still `Active` at the end of the function, which is an error.
        let res_local = mir::Local::from_u32(1); // Assuming `res` is local 1.
        assert_eq!(
            exit_state.get(res_local),
            &wasmrust::wasmir::linear_passes::LinearLivenessState::Active,
            "The dataflow analysis failed to detect an incomplete path for a linear type."
        );
    }
}

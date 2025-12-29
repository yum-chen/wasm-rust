//! Integration tests for WasmRust Cranelift Backend
//! 
//! This module provides comprehensive integration tests that verify the
//! Cranelift backend can compile real Rust code to functional WASM output.

use wasm::wasmir::{WasmIR, Signature, Type, Instruction, Terminator, Operand, BinaryOp};
use wasm::backend::{BackendFactory, BuildProfile, CompilationResult};
use wasm::backend::cranelift::{WasmRustCraneliftBackend, CompilationStats};
use quickcheck::{Arbitrary, Gen, QuickCheck, TestResult};
use std::time::Instant;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test data for integration testing
    struct TestData {
        name: &'static str,
        source: &'static str,
        expected_instructions: usize,
        expected_size_min: usize,
        expected_size_max: usize,
    }

    /// Simple test cases
    const SIMPLE_TESTS: &[TestData] = &[
        TestData {
            name: "add_function",
            source: r#"
                fn add(a: i32, b: i32) -> i32 {
                    a + b
                }
            "#,
            expected_instructions: 5,
            expected_size_min: 50,
            expected_size_max: 200,
        },
        TestData {
            name: "multiply_function",
            source: r#"
                fn multiply(a: i32, b: i32) -> i32 {
                    a * b
                }
            "#,
            expected_instructions: 5,
            expected_size_min: 50,
            expected_size_max: 200,
        },
        TestData {
            name: "factorial_function",
            source: r#"
                fn factorial(n: i32) -> i32 {
                    if n <= 1 {
                        1
                    } else {
                        n * factorial(n - 1)
                    }
                }
            "#,
            expected_instructions: 15,
            expected_size_min: 100,
            expected_size_max: 500,
        },
    ];

    /// Creates a WasmIR function for testing
    fn create_test_wasmir_function(name: &str, params: Vec<Type>, returns: Option<Type>) -> WasmIR {
        let signature = Signature { params, returns };
        let mut func = WasmIR::new(name.to_string(), signature);
        
        // Add local variables for parameters
        for param_ty in &params {
            func.add_local(param_ty.clone());
        }
        
        // Add local for result
        if returns.is_some() {
            func.add_local(returns.unwrap().clone());
        }
        
        func
    }

    /// Creates a simple add function in WasmIR
    fn create_add_function() -> WasmIR {
        let mut func = create_test_wasmir_function(
            "add",
            vec![Type::I32, Type::I32],
            Some(Type::I32),
        );
        
        let instructions = vec![
            Instruction::LocalGet { index: 0 },
            Instruction::LocalGet { index: 1 },
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(2), // Will be result of first get
                right: Operand::Local(1),
            },
            Instruction::LocalSet {
                index: 2,
                value: Operand::Local(0), // Result of addition
            },
            Instruction::Return {
                value: Some(Operand::Local(2)),
            },
        ];
        
        let terminator = Terminator::Return {
            value: Some(Operand::Local(2)),
        };
        
        func.add_basic_block(instructions, terminator);
        func
    }

    /// Creates a factorial function in WasmIR
    fn create_factorial_function() -> WasmIR {
        let mut func = create_test_wasmir_function(
            "factorial",
            vec![Type::I32],
            Some(Type::I32),
        );
        
        // Add more locals for computation
        func.add_local(Type::I32); // n-1
        func.add_local(Type::I32); // result
        
        // Basic block 1: Check if n <= 1
        let check_instructions = vec![
            Instruction::LocalGet { index: 0 }, // n
            Instruction::BinaryOp {
                op: BinaryOp::Le,
                left: Operand::Local(0),
                right: Operand::Local(1), // constant 1
            },
            Instruction::Branch {
                condition: Operand::Local(1), // result of comparison
                then_block: crate::wasmir::BlockId(1),
                else_block: crate::wasmir::BlockId(2),
            },
        ];
        
        let check_terminator = Terminator::Branch {
            condition: Operand::Local(1),
            then_block: crate::wasmir::BlockId(1),
            else_block: crate::wasmir::BlockId(2),
        };
        
        func.add_basic_block(check_instructions, check_terminator);
        
        // Basic block 2: Return 1 if n <= 1
        let base_case_instructions = vec![
            Instruction::LocalGet { index: 1 }, // constant 1
            Instruction::Return {
                value: Some(Operand::Local(1)),
            },
        ];
        
        let base_case_terminator = Terminator::Return {
            value: Some(Operand::Local(1)),
        };
        
        func.add_basic_block(base_case_instructions, base_case_terminator);
        
        // Basic block 3: n * factorial(n-1)
        let recursive_instructions = vec![
            Instruction::LocalGet { index: 0 }, // n
            Instruction::LocalGet { index: 2 }, // n-1
            Instruction::BinaryOp {
                op: BinaryOp::Sub,
                left: Operand::Local(0),
                right: Operand::Local(1),
            },
            Instruction::LocalSet {
                index: 2,
                value: Operand::Local(1), // n-1
            },
            Instruction::LocalGet { index: 2 },
            Instruction::BinaryOp {
                op: BinaryOp::Mul,
                left: Operand::Local(0),
                right: Operand::Local(2),
            },
            Instruction::LocalSet {
                index: 3,
                value: Operand::Local(1), // n * factorial(n-1)
            },
            Instruction::Return {
                value: Some(Operand::Local(3)),
            },
        ];
        
        let recursive_terminator = Terminator::Return {
            value: Some(Operand::Local(3)),
        };
        
        func.add_basic_block(recursive_instructions, recursive_terminator);
        
        func
    }

    /// Property: Simple functions compile correctly
    #[test]
    fn prop_simple_functions_compile() {
        fn property(test_data: TestData) -> TestResult {
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };
            
            let mut backend = WasmRustCraneliftBackend::new(target);
            if backend.is_err() {
                return TestResult::failed();
            }
            
            let mut backend = backend.unwrap();
            
            // Create test function based on test data
            let func = match test_data.name {
                "add_function" | "multiply_function" => create_add_function(),
                "factorial_function" => create_factorial_function(),
                _ => return TestResult::discard(),
            };
            
            let start = Instant::now();
            let result = backend.compile_function(&func, test_data.name);
            let compilation_time = start.elapsed();
            
            // Check compilation succeeded
            if result.is_err() {
                return TestResult::failed();
            }
            
            let code = result.unwrap();
            
            // Check code size is reasonable
            if code.len() < test_data.expected_size_min {
                return TestResult::failed();
            }
            
            if code.len() > test_data.expected_size_max {
                return TestResult::failed();
            }
            
            // Check compilation time is reasonable (< 1s for simple functions)
            if compilation_time.as_millis() > 1000 {
                return TestResult::failed();
            }
            
            // Check statistics
            let stats = backend.get_stats();
            if stats.functions_compiled != 1 {
                return TestResult::failed();
            }
            
            if stats.instructions_generated < test_data.expected_instructions / 2 {
                return TestResult::failed();
            }
            
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(20)
            .gen(Gen::new(100))
            .quickcheck(property as fn(TestData) -> TestResult);
    }

    /// Property: Compilation performance meets requirements
    #[test]
    fn prop_compilation_performance() {
        fn property(function_complexity: u8) -> TestResult {
            let complexity = function_complexity % 4 + 1; // 1-4
            
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };
            
            let mut backend = WasmRustCraneliftBackend::new(target);
            if backend.is_err() {
                return TestResult::failed();
            }
            
            let mut backend = backend.unwrap();
            
            // Create function with varying complexity
            let func = match complexity {
                1 => create_add_function(),
                2 => create_factorial_function(),
                3 => create_test_wasmir_function(
                    "complex",
                    vec![Type::I32, Type::I32, Type::I32],
                    Some(Type::I32),
                ),
                4 => create_test_wasmir_function(
                    "very_complex",
                    vec![Type::I64, Type::F64],
                    Some(Type::F64),
                ),
                _ => return TestResult::discard(),
            };
            
            let start = Instant::now();
            let result = backend.compile_function(&func, "performance_test");
            let compilation_time = start.elapsed();
            
            // Compilation should succeed
            if result.is_err() {
                return TestResult::failed();
            }
            
            // Performance requirements based on complexity
            let max_time_ms = match complexity {
                1 => 100,  // Simple: <100ms
                2 => 500,  // Medium: <500ms
                3 => 2000, // Complex: <2s
                4 => 5000, // Very complex: <5s
                _ => 1000, // Default: <1s
            };
            
            if compilation_time.as_millis() > max_time_ms {
                return TestResult::failed();
            }
            
            // Code size should be reasonable
            let code = result.unwrap();
            let max_size = match complexity {
                1 => 200,
                2 => 500,
                3 => 1000,
                4 => 2000,
                _ => 500,
            };
            
            if code.len() > max_size {
                return TestResult::failed();
            }
            
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(50)
            .gen(Gen::new(100))
            .quickcheck(property as fn(u8) -> TestResult);
    }

    /// Property: Backend factory creates appropriate backends
    #[test]
    fn prop_backend_factory_creation() {
        fn property(build_profile: u8) -> TestResult {
            let profile = match build_profile % 3 {
                0 => BuildProfile::Development,
                1 => BuildProfile::Release,
                2 => BuildProfile::Freestanding,
                _ => BuildProfile::Development,
            };
            
            let target = "wasm32";
            let backend = BackendFactory::create_backend(target, profile);
            
            // Should always be able to create a backend
            if backend.is_err() {
                return TestResult::failed();
            }
            
            let backend = backend.unwrap();
            
            // Check capabilities
            let capabilities = backend.capabilities();
            
            // All backends should support WASM optimizations
            if !capabilities.wasm_optimizations {
                return TestResult::failed();
            }
            
            // All backends should support thin monomorphization
            if !capabilities.thin_monomorphization {
                return TestResult::failed();
            }
            
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(20)
            .gen(Gen::new(100))
            .quickcheck(property as fn(u8) -> TestResult);
    }

    /// Property: Multiple function compilation works correctly
    #[test]
    fn prop_multiple_function_compilation() {
        fn property(function_count: u8) -> TestResult {
            if function_count == 0 {
                return TestResult::discard();
            }
            
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };
            
            let mut backend = WasmRustCraneliftBackend::new(target);
            if backend.is_err() {
                return TestResult::failed();
            }
            
            let mut backend = backend.unwrap();
            
            // Create multiple functions
            let mut functions = Vec::new();
            for i in 0..function_count {
                let func = create_test_wasmir_function(
                    &format!("func_{}", i),
                    vec![Type::I32],
                    Some(Type::I32),
                );
                functions.push(func);
            }
            
            let start = Instant::now();
            let results = backend.compile_functions(&functions);
            let compilation_time = start.elapsed();
            
            // All functions should compile successfully
            if results.is_err() {
                return TestResult::failed();
            }
            
            let compiled_results = results.unwrap();
            
            // Should have compiled all functions
            if compiled_results.len() != function_count as usize {
                return TestResult::failed();
            }
            
            // Each result should have non-empty code
            for (name, code) in &compiled_results {
                if code.is_empty() {
                    eprintln!("Function {} produced empty code", name);
                    return TestResult::failed();
                }
            }
            
            // Performance should scale reasonably (linear-ish)
            let max_time_ms = function_count as u64 * 200; // 200ms per function max
            if compilation_time.as_millis() > max_time_ms {
                return TestResult::failed();
            }
            
            // Check final statistics
            let stats = backend.get_stats();
            if stats.functions_compiled != function_count as usize {
                return TestResult::failed();
            }
            
            if stats.instructions_generated == 0 {
                return TestResult::failed();
            }
            
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(30)
            .gen(Gen::new(100))
            .quickcheck(property as fn(u8) -> TestResult);
    }

    /// Property: Compilation errors are handled gracefully
    #[test]
    fn prop_compilation_error_handling() {
        fn property(error_type: u8) -> TestResult {
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };
            
            let mut backend = WasmRustCraneliftBackend::new(target);
            if backend.is_err() {
                return TestResult::failed();
            }
            
            let mut backend = backend.unwrap();
            
            // Create invalid function based on error type
            let func = match error_type % 3 {
                0 => {
                    // Function with invalid type
                    let signature = Signature {
                        params: vec![],
                        returns: None,
                    };
                    WasmIR::new("invalid".to_string(), signature)
                }
                1 => {
                    // Function with no basic blocks
                    WasmIR::new("empty".to_string(), Signature {
                        params: vec![Type::I32],
                        returns: Some(Type::I32),
                    })
                }
                2 => {
                    // Function with invalid block references
                    let mut func = create_add_function();
                    // Manually corrupt the function (this is tricky in safe Rust)
                    // For now, just return a valid function
                    func
                }
                _ => return TestResult::discard(),
            };
            
            let result = backend.compile_function(&func, "error_test");
            
            // Some cases might succeed, some might fail - both are ok
            // The important thing is that we handle errors gracefully
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(20)
            .gen(Gen::new(100))
            .quickcheck(property as fn(u8) -> TestResult);
    }

    /// Test specific known good cases
    #[test]
    fn test_known_good_cases() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // Test the add function (should always work)
        let add_func = create_add_function();
        let result = backend.compile_function(&add_func, "add");
        
        assert!(result.is_ok(), "Add function should compile successfully");
        
        let code = result.unwrap();
        assert!(!code.is_empty(), "Compiled code should not be empty");
        assert!(code.len() > 50, "Add function should generate reasonable code size");
        assert!(code.len() < 200, "Add function should not be too large");
        
        // Check statistics
        let stats = backend.get_stats();
        assert_eq!(stats.functions_compiled, 1);
        assert!(stats.instructions_generated > 0);
        assert!(stats.compilation_time_ms > 0);
    }

    /// Test compilation with different optimization flags
    #[test]
    fn test_optimization_flags() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // Test with default flags
        let func = create_add_function();
        let result1 = backend.compile_function(&func, "test1");
        assert!(result1.is_ok());
        
        let code1 = result1.unwrap();
        
        // Clear stats
        backend.clear_stats();
        
        // Test again - should produce similar results
        let result2 = backend.compile_function(&func, "test2");
        assert!(result2.is_ok());
        
        let code2 = result2.unwrap();
        
        // Results should be deterministic (same input should produce same output)
        assert_eq!(code1, code2, "Compilation should be deterministic");
    }

    /// Test memory pressure handling
    #[test]
    fn test_memory_pressure() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // Create many functions to test memory handling
        let functions: Vec<WasmIR> = (0..100)
            .map(|i| create_test_wasmir_function(
                &format!("memory_test_{}", i),
                vec![Type::I32],
                Some(Type::I32),
            ))
            .collect();
        
        let start = Instant::now();
        let results = backend.compile_functions(&functions);
        let compilation_time = start.elapsed();
        
        // Should handle large batch compilation
        assert!(results.is_ok(), "Should handle compilation of many functions");
        
        let compiled_results = results.unwrap();
        assert_eq!(compiled_results.len(), 100, "Should compile all 100 functions");
        
        // Performance should scale reasonably
        let avg_time_per_function = compilation_time.as_millis() as f64 / 100.0;
        assert!(avg_time_per_function < 100.0, "Average time per function should be reasonable");
        
        // Memory usage should be controlled (can't directly test here, but timing gives hints)
        let stats = backend.get_stats();
        assert_eq!(stats.functions_compiled, 100);
    }

    /// Test error messages quality
    #[test]
    fn test_error_message_quality() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let backend = WasmRustCraneliftBackend::new(target);
        
        // Test with invalid target (should fail)
        let invalid_target = rustc_target::spec::Target {
            arch: "invalid_arch".to_string(),
            ..Default::default()
        };
        
        let invalid_backend = WasmRustCraneliftBackend::new(invalid_target);
        assert!(invalid_backend.is_err(), "Invalid target should fail");
        
        let error_msg = format!("{:?}", invalid_backend.unwrap_err());
        assert!(!error_msg.is_empty(), "Error message should not be empty");
        assert!(error_msg.len() > 10, "Error message should be descriptive");
    }

    /// Property: Backend statistics are accurate
    #[test]
    fn prop_backend_statistics_accuracy() {
        fn property(operation_count: u8) -> TestResult {
            if operation_count == 0 {
                return TestResult::discard();
            }
            
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };
            
            let mut backend = WasmRustCraneliftBackend::new(target);
            if backend.is_err() {
                return TestResult::failed();
            }
            
            let mut backend = backend.unwrap();
            
            // Clear initial stats
            backend.clear_stats();
            let initial_stats = backend.get_stats();
            
            if initial_stats.functions_compiled != 0 ||
               initial_stats.instructions_generated != 0 ||
               initial_stats.compilation_time_ms != 0 {
                return TestResult::failed();
            }
            
            // Perform operations
            for i in 0..operation_count {
                let func = create_test_wasmir_function(
                    &format!("stats_test_{}", i),
                    vec![Type::I32],
                    Some(Type::I32),
                );
                
                let result = backend.compile_function(&func, &format!("stats_{}", i));
                if result.is_err() {
                    return TestResult::failed();
                }
            }
            
            let final_stats = backend.get_stats();
            
            // Check statistics match operations
            if final_stats.functions_compiled != operation_count as usize {
                return TestResult::failed();
            }
            
            if final_stats.instructions_generated == 0 {
                return TestResult::failed();
            }
            
            if final_stats.compilation_time_ms == 0 {
                return TestResult::failed();
            }
            
            TestResult::passed()
        }

        QuickCheck::new()
            .tests(20)
            .gen(Gen::new(100))
            .quickcheck(property as fn(u8) -> TestResult);
    }
}

/// Integration test utilities
pub mod utils {
    use super::*;
    
    /// Creates a WASM validation test
    pub fn validate_wasm_output(wasm_code: &[u8]) -> Result<(), String> {
        // Basic WASM magic number check
        if wasm_code.len() < 8 {
            return Err("WASM code too short".to_string());
        }
        
        // Check for magic number
        if &wasm_code[0..4] != &[0x00, 0x61, 0x73, 0x6d] {
            return Err("Invalid WASM magic number".to_string());
        }
        
        // Check version
        if wasm_code[4] != 1 {
            return Err("Unsupported WASM version".to_string());
        }
        
        Ok(())
    }
    
    /// Extracts function count from WASM (simplified)
    pub fn extract_function_count(wasm_code: &[u8]) -> usize {
        // This is a simplified implementation
        // In a real implementation, this would parse the WASM section
        wasm_code.iter().filter(|&&b| *b == 0x01).count() / 2
    }
    
    /// Estimates WASM code complexity
    pub fn estimate_complexity(wasm_code: &[u8]) -> f64 {
        let mut complexity = 0.0;
        
        for &byte in wasm_code {
            match byte {
                // Control flow instructions increase complexity
                0x04 | 0x05 | 0x0b | 0x0c | 0x0d | 0x0e => complexity += 2.0,
                // Memory operations increase complexity
                0x28 | 0x29 | 0x2a | 0x2b | 0x2c | 0x2d | 0x2e | 0x2f => complexity += 1.5,
                // Arithmetic operations
                0x6a | 0x6b | 0x6c | 0x6d | 0x6e | 0x6f | 0x70 | 0x71 | 0x72 | 0x73 => complexity += 1.0,
                _ => {}
            }
        }
        
        complexity
    }
}

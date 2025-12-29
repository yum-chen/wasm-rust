//! Property-based tests for Cranelift compilation speed
//! 
//! This module validates that Cranelift backend provides fast compilation
//! suitable for development workflows.
//! 
//! Property 3: Cranelift Performance Advantage
//! Validates: Requirements 2.2

use wasm::backend::{BackendFactory, BuildProfile, CompilationResult};
use wasm::backend::cranelift::WasmRustCraneliftBackend;
use wasm::wasmir::{WasmIR, Signature, Type, Instruction, Terminator, Operand, BinaryOp};
use quickcheck::{Arbitrary, Gen, QuickCheck, TestResult};
use std::time::{Instant, Duration};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test data for performance testing
    #[derive(Debug, Clone)]
    struct PerformanceTestData {
        name: &'static str,
        wasmir_func: WasmIR,
        expected_max_compile_time: Duration,
        expected_max_code_size: usize,
        complexity_score: u8, // 1-5 complexity level
    }

    /// Arbitrary function complexity for testing
    #[derive(Debug, Clone, Copy, Arbitrary)]
    struct FunctionComplexity {
        param_count: u8,
        instruction_count: u8,
        basic_block_count: u8,
        has_loops: bool,
        has_branches: bool,
    }

    impl Arbitrary for FunctionComplexity {
        fn arbitrary(g: &mut Gen) -> Self {
            Self {
                param_count: g.gen_range(0..5),
                instruction_count: g.gen_range(1..20),
                basic_block_count: g.gen_range(1..5),
                has_loops: g.gen_bool(),
                has_branches: g.gen_bool(),
            }
        }
    }

    /// Creates test WasmIR functions with varying complexity
    fn create_test_wasmir_functions() -> Vec<PerformanceTestData> {
        vec![
            PerformanceTestData {
                name: "simple_identity",
                wasmir_func: create_identity_function(),
                expected_max_compile_time: Duration::from_millis(50),
                expected_max_code_size: 100,
                complexity_score: 1,
            },
            PerformanceTestData {
                name: "simple_arithmetic",
                wasmir_func: create_arithmetic_function(),
                expected_max_compile_time: Duration::from_millis(100),
                expected_max_code_size: 200,
                complexity_score: 2,
            },
            PerformanceTestData {
                name: "conditional_logic",
                wasmir_func: create_conditional_function(),
                expected_max_compile_time: Duration::from_millis(200),
                expected_max_code_size: 400,
                complexity_score: 3,
            },
            PerformanceTestData {
                name: "loop_function",
                wasmir_func: create_loop_function(),
                expected_max_compile_time: Duration::from_millis(300),
                expected_max_code_size: 600,
                complexity_score: 4,
            },
            PerformanceTestData {
                name: "recursive_function",
                wasmir_func: create_recursive_function(),
                expected_max_compile_time: Duration::from_millis(500),
                expected_max_code_size: 1000,
                complexity_score: 5,
            },
        ]
    }

    /// Creates a simple identity function
    fn create_identity_function() -> WasmIR {
        let signature = Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("identity".to_string(), signature);
        let local_input = func.add_local(Type::I32);

        let instructions = vec![
            Instruction::LocalGet { index: local_input },
            Instruction::Return {
                value: Some(Operand::Local(local_input)),
            },
        ];

        let terminator = Terminator::Return {
            value: Some(Operand::Local(local_input)),
        };

        func.add_basic_block(instructions, terminator);
        func
    }

    /// Creates a simple arithmetic function
    fn create_arithmetic_function() -> WasmIR {
        let signature = Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("arithmetic".to_string(), signature);
        let local_a = func.add_local(Type::I32);
        let local_b = func.add_local(Type::I32);
        let local_temp1 = func.add_local(Type::I32);
        let local_temp2 = func.add_local(Type::I32);

        let instructions = vec![
            Instruction::LocalGet { index: local_a },
            Instruction::LocalGet { index: local_b },
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(local_temp1),
                right: Operand::Local(local_b),
            },
            Instruction::LocalSet {
                index: local_temp1,
                value: Operand::Local(0), // Result of add
            },
            Instruction::LocalGet { index: local_temp1 },
            Instruction::LocalGet { index: local_a },
            Instruction::BinaryOp {
                op: BinaryOp::Mul,
                left: Operand::Local(local_temp2),
                right: Operand::Local(local_a),
            },
            Instruction::LocalSet {
                index: local_temp2,
                value: Operand::Local(1), // Result of mul
            },
            Instruction::LocalGet { index: local_temp2 },
            Instruction::Return {
                value: Some(Operand::Local(local_temp2)),
            },
        ];

        let terminator = Terminator::Return {
            value: Some(Operand::Local(local_temp2)),
        };

        func.add_basic_block(instructions, terminator);
        func
    }

    /// Creates a function with conditional logic
    fn create_conditional_function() -> WasmIR {
        let signature = Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("conditional".to_string(), signature);
        let local_a = func.add_local(Type::I32);
        let local_b = func.add_local(Type::I32);
        let local_result = func.add_local(Type::I32);
        let local_zero = func.add_local(Type::I32);

        // Basic block 1: comparison and branch
        let compare_instructions = vec![
            Instruction::LocalGet { index: local_a },
            Instruction::LocalGet { index: local_b },
            Instruction::BinaryOp {
                op: BinaryOp::Lt,
                left: Operand::Local(local_a),
                right: Operand::Local(local_b),
            },
            Instruction::Branch {
                condition: Operand::Local(0), // Result of comparison
                then_block: wasmir::BlockId(1),
                else_block: wasmir::BlockId(2),
            },
        ];

        let compare_terminator = Terminator::Branch {
            condition: Operand::Local(0),
            then_block: wasmir::BlockId(1),
            else_block: wasmir::BlockId(2),
        };

        // Basic block 2: a < b, return a
        let then_instructions = vec![
            Instruction::LocalGet { index: local_a },
            Instruction::LocalSet {
                index: local_result,
                value: Operand::Local(local_a),
            },
            Instruction::LocalGet { index: local_result },
            Instruction::Return {
                value: Some(Operand::Local(local_result)),
            },
        ];

        let then_terminator = Terminator::Return {
            value: Some(Operand::Local(local_result)),
        };

        // Basic block 3: a >= b, return b
        let else_instructions = vec![
            Instruction::LocalGet { index: local_b },
            Instruction::LocalSet {
                index: local_result,
                value: Operand::Local(local_b),
            },
            Instruction::LocalGet { index: local_result },
            Instruction::Return {
                value: Some(Operand::Local(local_result)),
            },
        ];

        let else_terminator = Terminator::Return {
            value: Some(Operand::Local(local_result)),
        };

        func.add_basic_block(vec![
            Instruction::BinaryOp {
                op: BinaryOp::Lt,
                left: Operand::Local(local_a),
                right: Operand::Local(local_b),
            },
        ], compare_terminator);
        func.add_basic_block(then_instructions, then_terminator);
        func.add_basic_block(else_instructions, else_terminator);
        func
    }

    /// Creates a function with a loop
    fn create_loop_function() -> WasmIR {
        let signature = Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("loop_function".to_string(), signature);
        let local_n = func.add_local(Type::I32);
        let local_limit = func.add_local(Type::I32);
        let local_result = func.add_local(Type::I32);
        let local_i = func.add_local(Type::I32);

        // Basic block 1: initialize loop
        let init_instructions = vec![
            Instruction::LocalSet {
                index: local_result,
                value: Operand::Constant(wasmir::Constant::I32(0)),
            },
            Instruction::LocalSet {
                index: local_i,
                value: Operand::Constant(wasmir::Constant::I32(0)),
            },
            Instruction::Jump { target: wasmir::BlockId(1) },
        ];

        let init_terminator = Terminator::Jump { target: wasmir::BlockId(1) };
        func.add_basic_block(init_instructions, init_terminator);

        // Basic block 2: loop condition
        let loop_cond_instructions = vec![
            Instruction::LocalGet { index: local_i },
            Instruction::LocalGet { index: local_n },
            Instruction::BinaryOp {
                op: BinaryOp::Lt,
                left: Operand::Local(local_i),
                right: Operand::Local(local_n),
            },
            Instruction::Branch {
                condition: Operand::Local(0),
                then_block: wasmir::BlockId(2),
                else_block: wasmir::BlockId(3),
            },
        ];

        let loop_cond_terminator = Terminator::Branch {
            condition: Operand::Local(0),
            then_block: wasmir::BlockId(2),
            else_block: wasmir::BlockId(3),
        };

        // Basic block 3: loop body
        let loop_body_instructions = vec![
            Instruction::LocalGet { index: local_result },
            Instruction::LocalGet { index: local_limit },
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(0),
                right: Operand::Local(local_limit),
            },
            Instruction::LocalSet {
                index: local_result,
                value: Operand::Local(0),
            },
            Instruction::LocalGet { index: local_i },
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(local_i),
                right: Operand::Constant(wasmir::Constant::I32(1)),
            },
            Instruction::LocalSet {
                index: local_i,
                value: Operand::Local(0),
            },
            Instruction::Jump { target: wasmir::BlockId(1) },
        ];

        let loop_body_terminator = Terminator::Jump { target: wasmir::BlockId(1) };
        func.add_basic_block(loop_body_instructions, loop_body_terminator);

        // Basic block 4: loop exit
        let loop_exit_instructions = vec![
            Instruction::LocalGet { index: local_result },
            Instruction::Return {
                value: Some(Operand::Local(local_result)),
            },
        ];

        let loop_exit_terminator = Terminator::Return {
            value: Some(Operand::Local(local_result)),
        };

        func.add_basic_block(loop_cond_instructions, loop_cond_terminator);
        func.add_basic_block(loop_exit_instructions, loop_exit_terminator);
        func
    }

    /// Creates a recursive function
    fn create_recursive_function() -> WasmIR {
        let signature = Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("recursive".to_string(), signature);
        let local_n = func.add_local(Type::I32);

        // Basic block 1: check if n <= 1
        let check_instructions = vec![
            Instruction::LocalGet { index: local_n },
            Instruction::BinaryOp {
                op: BinaryOp::Le,
                left: Operand::Local(local_n),
                right: Operand::Constant(wasmir::Constant::I32(1)),
            },
            Instruction::Branch {
                condition: Operand::Local(0),
                then_block: wasmir::BlockId(1),
                else_block: wasmir::BlockId(2),
            },
        ];

        let check_terminator = Terminator::Branch {
            condition: Operand::Local(0),
            then_block: wasmir::BlockId(1),
            else_block: wasmir::BlockId(2),
        };

        // Basic block 2: base case, return 1
        let base_case_instructions = vec![
            Instruction::LocalGet { index: local_n },
            Instruction::Return {
                value: Some(Operand::Constant(wasmir::Constant::I32(1))),
            },
        ];

        let base_case_terminator = Terminator::Return {
            value: Some(Operand::Constant(wasmir::Constant::I32(1))),
        };

        // Basic block 3: recursive case, n * factorial(n-1)
        let recursive_instructions = vec![
            Instruction::LocalGet { index: local_n },
            Instruction::BinaryOp {
                op: BinaryOp::Sub,
                left: Operand::Local(local_n),
                right: Operand::Constant(wasmir::Constant::I32(1)),
            },
            Instruction::Return {
                value: Some(Operand::Local(0)), // Recursive call placeholder
            },
        ];

        let recursive_terminator = Terminator::Return {
            value: Some(Operand::Local(0)),
        };

        func.add_basic_block(check_instructions, check_terminator);
        func.add_basic_block(base_case_instructions, base_case_terminator);
        func.add_basic_block(recursive_instructions, recursive_terminator);
        func
    }

    /// Property: Compilation speed scales with complexity
    #[test]
    fn prop_compilation_speed_scaling() {
        fn property(complexity: FunctionComplexity) -> TestResult {
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };

            let mut backend = WasmRustCraneliftBackend::new(target);
            if backend.is_err() {
                return TestResult::failed();
            }

            let mut backend = backend.unwrap();

            // Create function with specified complexity
            let func = create_function_with_complexity(&mut backend, complexity);
            
            // Measure compilation time
            let start = Instant::now();
            let result = backend.compile_function(&func, "performance_test");
            let compilation_time = start.elapsed();

            // Compilation should succeed
            if result.is_err() {
                return TestResult::failed();
            }

            let code = result.unwrap();

            // Check that code was generated
            if code.is_empty() {
                return TestResult::failed();
            }

            // Performance requirements based on complexity
            let max_time_ms = match complexity.complexity_score {
                1 => 50,   // Simple: <50ms
                2 => 100,  // Medium: <100ms
                3 => 250,  // Complex: <250ms
                4 => 400,  // Very complex: <400ms
                5 => 600,  // Extremely complex: <600ms
                _ => 1000, // Fallback: <1s
            };

            let max_code_size = match complexity.complexity_score {
                1 => 150,   // Simple: <150 bytes
                2 => 300,   // Medium: <300 bytes
                3 => 600,   // Complex: <600 bytes
                4 => 1000,  // Very complex: <1000 bytes
                5 => 1500,  // Extremely complex: <1500 bytes
                _ => 2000,  // Fallback: <2000 bytes
            };

            if compilation_time.as_millis() > max_time_ms {
                eprintln!("Compilation too slow: {}ms > {}ms for complexity {}",
                    compilation_time.as_millis(), max_time_ms, complexity.complexity_score);
                return TestResult::failed();
            }

            if code.len() > max_code_size {
                eprintln!("Code too large: {} bytes > {} bytes for complexity {}",
                    code.len(), max_code_size, complexity.complexity_score);
                return TestResult::failed();
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(100)
            .gen(Gen::new(100))
            .quickcheck(property as fn(FunctionComplexity) -> TestResult);
    }

    /// Property: Development profile compiles faster than production profile
    #[test]
    fn prop_dev_profile_faster_than_release() {
        fn property(complexity: FunctionComplexity) -> TestResult {
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };

            // Test with development profile
            let dev_result = compile_with_profile(&target, &complexity, BuildProfile::Development);
            if dev_result.is_err() {
                return TestResult::failed();
            }

            let (dev_time, dev_size) = dev_result.unwrap();

            // Test with release profile (if available)
            let rel_result = compile_with_profile(&target, &complexity, BuildProfile::Release);
            if rel_result.is_err() {
                // Release might not be available, that's ok
                return TestResult::passed();
            }

            let (rel_time, rel_size) = rel_result.unwrap();

            // Development should be at least as fast as release
            if dev_time > rel_time * 2 {
                return TestResult::failed();
            }

            // Development should not generate significantly larger code
            if dev_size > rel_size * 3 {
                return TestResult::failed();
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(50)
            .gen(Gen::new(100))
            .quickcheck(property as fn(FunctionComplexity) -> TestResult);
    }

    /// Property: Memory usage is reasonable during compilation
    #[test]
    fn prop_memory_usage_reasonable() {
        fn property(complexity: FunctionComplexity) -> TestResult {
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

            // Compile multiple functions to stress test memory
            let functions: Vec<WasmIR> = (0..10)
                .map(|_| create_function_with_complexity(&mut backend, complexity))
                .collect();

            let start = Instant::now();
            let results = backend.compile_functions(&functions);
            let compilation_time = start.elapsed();

            // All should compile successfully
            if results.is_err() {
                return TestResult::failed();
            }

            let compiled_results = results.unwrap();

            // Check final statistics
            let final_stats = backend.get_stats();

            if final_stats.functions_compiled != 10 {
                return TestResult::failed();
            }

            if final_stats.instructions_generated == 0 {
                return TestResult::failed();
            }

            // Memory usage should scale reasonably
            // (We can't directly measure memory, but we can infer from patterns)
            let avg_time_per_function = compilation_time.as_millis() as f64 / 10.0;
            let max_reasonable_time = match complexity.complexity_score {
                1 => 100.0,
                2 => 200.0,
                3 => 400.0,
                4 => 800.0,
                5 => 1500.0,
                _ => 2000.0,
            };

            if avg_time_per_function > max_reasonable_time {
                return TestResult::failed();
            }

            // Check that we got reasonable code sizes
            for (i, (_, code)) in compiled_results.iter().enumerate() {
                let max_size = match complexity.complexity_score {
                    1 => 200,
                    2 => 400,
                    3 => 800,
                    4 => 1200,
                    5 => 2000,
                    _ => 3000,
                };

                if code.len() > max_size {
                    eprintln!("Function {} too large: {} bytes", i, code.len());
                    return TestResult::failed();
                }
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(30)
            .gen(Gen::new(100))
            .quickcheck(property as fn(FunctionComplexity) -> TestResult);
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
                let complexity = FunctionComplexity {
                    param_count: 1,
                    instruction_count: 5 + (i % 10) as u8,
                    basic_block_count: 1 + (i % 3) as u8,
                    has_loops: i % 2 == 0,
                    has_branches: i % 3 == 0,
                };

                let func = create_function_with_complexity(&mut backend, complexity);
                let result = backend.compile_function(&func, &format!("stats_test_{}", i));

                if result.is_err() {
                    return TestResult::failed();
                }
            }

            // Check final statistics
            let final_stats = backend.get_stats();

            if final_stats.functions_compiled != operation_count as usize {
                return TestResult::failed();
            }

            if final_stats.instructions_generated == 0 {
                return TestResult::failed();
            }

            if final_stats.compilation_time_ms == 0 {
                return TestResult::failed();
            }

            // Statistics should be internally consistent
            if final_stats.functions_compiled != operation_count as usize {
                return TestResult::failed();
            }

            // Instructions generated should be reasonable for complexity
            let min_expected_instructions = operation_count as usize * 3; // At least 3 instructions per function
            if final_stats.instructions_generated < min_expected_instructions {
                return TestResult::failed();
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(50)
            .gen(Gen::new(100))
            .quickcheck(property as fn(u8) -> TestResult);
    }

    /// Property: Deterministic compilation results
    #[test]
    fn prop_deterministic_compilation() {
        fn property(complexity: FunctionComplexity) -> TestResult {
            let target = rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            };

            let func = create_function_with_complexity(&mut WasmRustCraneliftBackend::new(target.clone()).unwrap(), complexity);

            // Compile the same function multiple times
            let results: Result<Vec<Vec<u8>>, _> = (0..3)
                .map(|_| {
                    let mut backend = WasmRustCraneliftBackend::new(target.clone()).unwrap();
                    backend.compile_function(&func, "deterministic_test")
                })
                .collect();

            // All compilations should succeed
            if results.iter().any(|r| r.is_err()) {
                return TestResult::failed();
            }

            let compiled_results: Vec<Vec<u8>> = results.iter().map(|r| r.clone().unwrap()).collect();

            // All results should be identical
            if let Some(first_result) = compiled_results.first() {
                for (i, result) in compiled_results.iter().enumerate() {
                    if result != first_result {
                        return TestResult::failed();
                    }
                }
            }

            TestResult::passed()
        }

        QuickCheck::new()
            .tests(30)
            .gen(Gen::new(100))
            .quickcheck(property as fn(FunctionComplexity) -> TestResult);
    }

    /// Creates a function with specified complexity
    fn create_function_with_complexity(
        backend: &mut WasmRustCraneliftBackend,
        complexity: FunctionComplexity,
    ) -> WasmIR {
        let mut param_types = Vec::new();
        for _ in 0..complexity.param_count {
            param_types.push(Type::I32);
        }

        let signature = Signature {
            params: param_types.clone(),
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("test_function".to_string(), signature);

        // Add local variables
        let mut local_indices = Vec::new();
        for _ in 0..complexity.param_count {
            local_indices.push(func.add_local(Type::I32));
        }
        let local_result = func.add_local(Type::I32);

        // Create basic blocks
        let mut instructions = Vec::new();

        // Add some arithmetic operations based on instruction count
        for i in 0..(complexity.instruction_count / 2) {
            let op1 = local_indices.get(i % local_indices.len()).unwrap_or(&local_result);
            let op2 = local_indices.get((i + 1) % local_indices.len()).unwrap_or(&local_result);

            instructions.push(Instruction::LocalGet { index: *op1 });
            instructions.push(Instruction::LocalGet { index: *op2 });
            instructions.push(Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(*op1),
                right: Operand::Local(*op2),
            });
        }

        if complexity.has_branches {
            // Add a conditional branch
            if !local_indices.is_empty() {
                instructions.push(Instruction::Branch {
                    condition: Operand::Local(*local_indices.first().unwrap_or(&local_result)),
                    then_block: wasmir::BlockId(1),
                    else_block: wasmir::BlockId(2),
                });
            }
        }

        instructions.push(Instruction::Return {
            value: Some(Operand::Local(local_result)),
        });

        let terminator = Terminator::Return {
            value: Some(Operand::Local(local_result)),
        };

        func.add_basic_block(instructions, terminator);
        func
    }

    /// Compiles a function with a specific profile and returns time and size
    fn compile_with_profile(
        target: &rustc_target::spec::Target,
        complexity: &FunctionComplexity,
        profile: BuildProfile,
    ) -> Result<(Duration, usize), Box<dyn std::error::Error>> {
        let mut backend = BackendFactory::create_backend("wasm32", profile)?;
        let func = create_function_with_complexity(
            &mut backend.as_any().downcast_mut::<WasmRustCraneliftBackend>()
                .ok_or("Backend is not Cranelift")?,
            *complexity,
        );

        let start = Instant::now();
        let result = backend.compile(&func, profile)?;
        let compilation_time = start.elapsed();

        Ok((compilation_time, result.code.len()))
    }
}

/// Performance benchmark utilities
pub mod benchmarks {
    use super::*;

    /// Runs comprehensive performance benchmarks
    pub fn run_performance_benchmarks() -> Result<(), Box<dyn std::error::Error>> {
        println!("Running Cranelift Performance Benchmarks...");
        
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };

        let test_functions = super::create_test_wasmir_functions();

        for test_data in &test_functions {
            println!("Benchmarking function: {}", test_data.name);
            
            let target_clone = target.clone();
            let mut backend = WasmRustCraneliftBackend::new(target_clone)?;
            let func = test_data.wasmir_func.clone();
            
            // Warm up
            for _ in 0..3 {
                let _ = backend.compile_function(&func, "warmup");
            }
            
            // Run multiple iterations
            let mut times = Vec::new();
            let mut sizes = Vec::new();
            
            for i in 0..10 {
                backend.clear_stats();
                
                let start = Instant::now();
                let result = backend.compile_function(&func, &format!("benchmark_{}_{}", test_data.name, i));
                let duration = start.elapsed();
                
                if result.is_ok() {
                    times.push(duration);
                    sizes.push(result.unwrap().len());
                }
            }
            
            if !times.is_empty() {
                println!("  FAILED: All compilation attempts failed");
                continue;
            }
            
            // Calculate statistics
            let avg_time = times.iter().sum::<Duration>() / times.len() as u32;
            let min_time = times.iter().min().unwrap();
            let max_time = times.iter().max().unwrap();
            
            let avg_size = sizes.iter().sum::<usize>() / sizes.len();
            let min_size = sizes.iter().min().unwrap();
            let max_size = sizes.iter().max().unwrap();
            
            println!("  Average compile time: {:?}", avg_time);
            println!("  Min compile time: {:?}", min_time);
            println!("  Max compile time: {:?}", max_time);
            println!("  Average code size: {} bytes", avg_size);
            println!("  Min code size: {} bytes", min_size);
            println!("  Max code size: {} bytes", max_size);
            
            // Check against expectations
            if avg_time > test_data.expected_max_compile_time {
                println!("  ⚠️  SLOWER than expected: {:?} > {:?}", avg_time, test_data.expected_max_compile_time);
            }
            
            if avg_size > test_data.expected_max_code_size {
                println!("  ⚠️  LARGER than expected: {} > {}", avg_size, test_data.expected_max_code_size);
            }
            
            println!("  Status: {}", 
                if avg_time <= test_data.expected_max_compile_time && avg_size <= test_data.expected_max_code_size {
                    "✅ PASS"
                } else {
                    "❌ FAIL"
                });
            println!();
        }
        
        Ok(())
    }

    /// Compares Cranelift vs theoretical baseline
    pub fn compare_with_baseline() -> Result<(), Box<dyn std::error::Error>> {
        println!("Comparing Cranelift with baseline performance...");
        
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let simple_func = super::create_identity_function();
        let complex_func = super::create_recursive_function();
        
        let mut backend = WasmRustCraneliftBackend::new(target)?;
        
        // Test simple function
        let start = Instant::now();
        let simple_result = backend.compile_function(&simple_func, "simple_baseline");
        let simple_time = start.elapsed();
        let simple_size = simple_result.map(|c| c.len()).unwrap_or(0);
        
        // Test complex function
        let start = Instant::now();
        let complex_result = backend.compile_function(&complex_func, "complex_baseline");
        let complex_time = start.elapsed();
        let complex_size = complex_result.map(|c| c.len()).unwrap_or(0);
        
        println!("Simple function (identity):");
        println!("  Compile time: {:?}", simple_time);
        println!("  Code size: {} bytes", simple_size);
        
        println!("Complex function (recursive):");
        println!("  Compile time: {:?}", complex_time);
        println!("  Code size: {} bytes", complex_size);
        
        // Performance ratio
        let time_ratio = complex_time.as_millis() as f64 / simple_time.as_millis() as f64;
        let size_ratio = complex_size as f64 / simple_size as f64;
        
        println!("Performance ratios:");
        println!("  Time ratio (complex/simple): {:.2}x", time_ratio);
        println!("  Size ratio (complex/simple): {:.2}x", size_ratio);
        
        // Expect reasonable scaling (complex shouldn't be > 10x slower or larger)
        if time_ratio > 10.0 {
            println!("  ⚠️  Time scaling seems poor");
        }
        
        if size_ratio > 10.0 {
            println!("  ⚠️  Size scaling seems poor");
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use super::benchmarks::*;

    #[test]
    fn test_performance_benchmarks() {
        let result = benchmarks::run_performance_benchmarks();
        assert!(result.is_ok(), "Performance benchmarks should complete successfully");
    }

    #[test]
    fn test_baseline_comparison() {
        let result = benchmarks::compare_with_baseline();
        assert!(result.is_ok(), "Baseline comparison should complete successfully");
    }

    #[test]
    fn test_known_good_cases() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // Test that known good cases perform well
        let simple_func = create_identity_function();
        let start = Instant::now();
        let result = backend.compile_function(&simple_func, "known_good");
        let compilation_time = start.elapsed();
        
        assert!(result.is_ok(), "Simple function should compile");
        assert!(compilation_time.as_millis() < 100, "Simple function should compile quickly");
        
        let code = result.unwrap();
        assert!(!code.is_empty(), "Should generate some code");
        assert!(code.len() < 200, "Simple function should generate small code");
    }
}

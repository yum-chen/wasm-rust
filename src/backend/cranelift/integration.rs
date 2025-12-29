//! Integration tests for Cranelift backend
//! 
//! This module provides integration tests that verify the Cranelift backend
//! can compile actual Rust code to functional WASM output.

use super::*;
use crate::wasmir::{WasmIR, Signature, Type, Instruction, Terminator, Operand, BinaryOp};
use std::collections::HashMap;

/// Integration test runner for Cranelift backend
pub struct CraneliftIntegrationTest;

impl CraneliftIntegrationTest {
    /// Creates a new integration test instance
    pub fn new() -> Self {
        Self {}
    }

    /// Tests compilation of a simple function
    pub fn test_simple_function_compilation(&self) -> Result<(), String> {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target)
            .map_err(|e| format!("Failed to create backend: {}", e))?;

        // Create a simple WasmIR function: add(i32, i32) -> i32
        let signature = Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("add".to_string(), signature);
        let local_a = func.add_local(Type::I32);
        let local_b = func.add_local(Type::I32);
        let local_result = func.add_local(Type::I32);

        // Function body
        let instructions = vec![
            Instruction::LocalGet { index: local_a },
            Instruction::LocalGet { index: local_b },
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(local_result),
                right: Operand::Local(local_b),
            },
            Instruction::LocalSet { 
                index: local_result,
                value: Operand::Local(0), // Result of addition
            },
            Instruction::LocalGet { index: local_result },
            Instruction::Return { 
                value: Some(Operand::Local(local_result))
            },
        ];

        let terminator = Terminator::Return {
            value: Some(Operand::Local(local_result)),
        };

        func.add_basic_block(instructions, terminator);

        // Compile the function
        let result = backend.compile_function(&func, "add")
            .map_err(|e| format!("Compilation failed: {}", e))?;

        // Verify the result
        if result.is_empty() {
            return Err("Generated empty code".to_string());
        }

        // Basic validation: check for expected instruction patterns
        let has_add_instruction = result.iter().any(|&byte| {
            // This is a simplified check - in real WASM, we'd look for specific opcodes
            // For now, just check that we got some non-trivial output
            *byte != 0x00
        });

        if !has_add_instruction {
            return Err("Generated code doesn't contain expected patterns".to_string());
        }

        // Check that we have a reasonable amount of code
        if result.len() < 10 {
            return Err("Generated code too small".to_string());
        }

        if result.len() > 1000 {
            return Err("Generated code too large".to_string());
        }

        // Check compilation statistics
        let stats = backend.get_stats();
        if stats.functions_compiled != 1 {
            return Err(format!("Expected 1 compiled function, got {}", stats.functions_compiled));
        }

        if stats.instructions_generated == 0 {
            return Err("No instructions generated".to_string());
        }

        // Check performance targets (simplified)
        let compilation_time = stats.compilation_time_ms;
        if compilation_time > 1000 { // 1 second
            return Err(format!("Compilation took too long: {}ms", compilation_time));
        }

        Ok(())
    }

    /// Tests compilation of a function with control flow
    pub fn test_control_flow_compilation(&self) -> Result<(), String> {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target)
            .map_err(|e| format!("Failed to create backend: {}", e))?;

        // Create a function with conditional logic
        let signature = Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("abs".to_string(), signature);
        let local_input = func.add_local(Type::I32);
        let local_zero = func.add_local(Type::I32);
        let local_result = func.add_local(Type::I32);

        // Function body: if input < 0, return -input, else return input
        let instructions = vec![
            Instruction::LocalGet { index: local_input },
            Instruction::LocalGet { index: local_zero },
            Instruction::BinaryOp {
                op: BinaryOp::Lt,
                left: Operand::Local(local_input),
                right: Operand::Local(local_zero),
            },
            Instruction::Branch {
                condition: Operand::Local(2), // Result of comparison
                then_block: crate::wasmir::BlockId(1),
                else_block: crate::wasmir::BlockId(2),
            },
        ];

        // Basic block: return -input
        let then_instructions = vec![
            Instruction::LocalGet { index: local_input },
            Instruction::UnaryOp {
                op: crate::wasmir::UnaryOp::Neg,
                value: Operand::Local(local_input),
            },
            Instruction::Return {
                value: Some(Operand::Local(1)),
            },
        ];
        let then_terminator = Terminator::Return {
            value: Some(Operand::Local(1)),
        };

        // Else block: return input
        let else_instructions = vec![
            Instruction::LocalGet { index: local_input },
            Instruction::Return {
                value: Some(Operand::Local(local_input)),
            },
        ];
        let else_terminator = Terminator::Return {
            value: Some(Operand::Local(local_input)),
        };

        func.add_basic_block(vec![
            Instruction::BinaryOp {
                op: BinaryOp::Lt,
                left: Operand::Local(local_input),
                right: Operand::Local(local_zero),
            },
        ], Terminator::Branch {
            condition: Operand::Local(2),
            then_block: crate::wasmir::BlockId(1),
            else_block: crate::wasmir::BlockId(2),
        });
        func.add_basic_block(then_instructions, then_terminator);
        func.add_basic_block(else_instructions, else_terminator);

        // Compile the function
        let result = backend.compile_function(&func, "abs")
            .map_err(|e| format!("Compilation failed: {}", e))?;

        // Verify the result
        if result.is_empty() {
            return Err("Generated empty code".to_string());
        }

        // Should have more complex control flow (branching)
        if result.len() < 20 {
            return Err("Generated code too simple for control flow".to_string());
        }

        // Should have multiple basic blocks
        let stats = backend.get_stats();
        if stats.instructions_generated < 15 {
            return Err("Not enough instructions generated for control flow".to_string());
        }

        Ok(())
    }

    /// Tests compilation of a memory access function
    pub fn test_memory_access_compilation(&self) -> Result<(), String> {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target)
            .map_err(|e| format!("Failed to create backend: {}", e))?;

        // Create a function that accesses memory
        let signature = Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        };

        let mut func = WasmIR::new("store_and_load".to_string(), signature);
        let local_addr = func.add_local(Type::I32);
        let local_value = func.add_local(Type::I32);
        let local_temp = func.add_local(Type::I32);

        let instructions = vec![
            Instruction::LocalGet { index: local_addr },
            Instruction::LocalGet { index: local_value },
            Instruction::MemoryStore {
                address: Operand::Local(local_addr),
                value: Operand::Local(local_temp),
                ty: Type::I32,
                align: None,
                offset: 0,
            },
            Instruction::LocalGet { index: local_addr },
            Instruction::MemoryLoad {
                address: Operand::Local(local_addr),
                ty: Type::I32,
                align: None,
                offset: 0,
            },
            Instruction::Return {
                value: Some(Operand::Local(local_temp)),
            },
        ];

        let terminator = Terminator::Return {
            value: Some(Operand::Local(local_temp)),
        };

        func.add_basic_block(instructions, terminator);

        // Compile the function
        let result = backend.compile_function(&func, "store_and_load")
            .map_err(|e| format!("Compilation failed: {}", e))?;

        // Verify the result contains memory operations
        let has_memory_ops = result.iter().any(|&byte| {
            // Simplified check for memory-like patterns
            *byte == 0x28 || *byte == 0x2a || *byte == 0x28 // WASM opcodes
        });

        if !has_memory_ops {
            return Err("Generated code doesn't contain memory operations".to_string());
        }

        // Should be a reasonable size
        if result.len() < 20 {
            return Err("Memory function code too small".to_string());
        }

        Ok(())
    }

    /// Runs all integration tests
    pub fn run_all_tests(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        let tests = vec![
            ("simple_function", Self::test_simple_function_compilation),
            ("control_flow", Self::test_control_flow_compilation),
            ("memory_access", Self::test_memory_access_compilation),
        ];

        for (test_name, test_fn) in tests {
            println!("Running integration test: {}", test_name);
            if let Err(e) = test_fn(self) {
                errors.push(format!("Test '{}' failed: {}", test_name, e));
            } else {
                println!("Test '{}' passed", test_name);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function() {
        let test_runner = CraneliftIntegrationTest::new();
        
        if let Err(errors) = test_runner.run_all_tests() {
            for error in errors {
                eprintln!("Integration test error: {}", error);
            }
            panic!("Some integration tests failed");
        }
    }

    #[test]
    fn test_backend_statistics() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // Test initial statistics
        let stats = backend.get_stats();
        assert_eq!(stats.functions_compiled, 0);
        assert_eq!(stats.instructions_generated, 0);
        assert_eq!(stats.compilation_time_ms, 0);
        
        // Compile a function and check statistics update
        let signature = crate::wasmir::Signature {
            params: vec![crate::wasmir::Type::I32],
            returns: Some(crate::wasmir::Type::I32),
        };
        
        let func = crate::wasmir::WasmIR::new("test".to_string(), signature);
        let result = backend.compile_function(&func, "test");
        assert!(result.is_ok());
        
        let updated_stats = backend.get_stats();
        assert_eq!(updated_stats.functions_compiled, 1);
        assert!(updated_stats.instructions_generated > 0);
        assert!(updated_stats.compilation_time_ms > 0);
        
        // Reset and verify
        backend.clear_stats();
        let cleared_stats = backend.get_stats();
        assert_eq!(cleared_stats.functions_compiled, 0);
        assert_eq!(cleared_stats.instructions_generated, 0);
        assert_eq!(cleared_stats.compilation_time_ms, 0);
    }

    #[test]
    fn test_multiple_functions() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // Create multiple functions
        let mut functions = Vec::new();
        for i in 0..3 {
            let signature = crate::wasmir::Signature {
                params: vec![crate::wasmir::Type::I32],
                returns: Some(crate::wasmir::Type::I32),
            };
            
            let mut func = crate::wasmir::WasmIR::new(format!("func_{}", i), signature);
            let local_param = func.add_local(crate::wasmir::Type::I32);
            let local_result = func.add_local(crate::wasmir::Type::I32);
            
            let instructions = vec![
                crate::wasmir::Instruction::LocalGet { index: local_param },
                crate::wasmir::Instruction::LocalSet { 
                    index: local_result,
                    value: crate::wasmir::Operand::Local(local_param),
                },
                crate::wasmir::Instruction::LocalGet { index: local_result },
                crate::wasmir::Instruction::Return { 
                    value: Some(crate::wasmir::Operand::Local(local_result)),
                },
            ];
            
            let terminator = crate::wasmir::Terminator::Return {
                value: Some(crate::wasmir::Operand::Local(local_result)),
            };
            
            func.add_basic_block(instructions, terminator);
            functions.push(func);
        }
        
        let results = backend.compile_functions(&functions);
        assert!(results.is_ok());
        
        let compiled_results = results.unwrap();
        assert_eq!(compiled_results.len(), 3);
        
        for (i, (name, code)) in compiled_results.iter().enumerate() {
            assert!(name.starts_with("func_"));
            assert!(!code.is_empty());
        }
    }

    #[test]
    fn test_optimization_flags() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // The backend should have default optimization flags
        // We can't directly access the flags since they're private,
        // but we can test that the backend functions correctly
        let signature = crate::wasmir::Signature {
            params: vec![crate::wasmir::Type::I32],
            returns: Some(crate::wasmir::Type::I32),
        };
        
        let func = crate::wasmir::WasmIR::new("test".to_string(), signature);
        let result = backend.compile_function(&func, "test");
        assert!(result.is_ok());
    }
}

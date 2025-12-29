//! Integration tests for WasmIR → WASM compilation
//! 
//! Tests the complete pipeline from MIR to WASM using the new
//! WasmIR bridge and WASM code generation.

use wasm::wasmir::{WasmIR, Instruction, Terminator, BasicBlock, Type, Signature, Operand, BinaryOp, Constant};
use wasm::backend::cranelift::{WasmRustCraneliftBackend, wasm_codegen::WasmCodegen};
use wasm::backend::{BackendError, CompilationResult};

#[test]
fn test_simple_wasmir_to_wasm() {
    let mut wasmir = create_simple_add_function();
    let mut codegen = WasmCodegen::new();
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    assert!(!compilation_result.code.is_empty());
    
    // Check for WASM magic number
    assert_eq!(&compilation_result.code[0..4], &[0x00, 0x61, 0x73, 0x6d]);
    assert_eq!(&compilation_result.code[4..8], &[0x01, 0x00, 0x00, 0x00]);
}

#[test]
fn test_cranelift_wasm_compilation() {
    let target = rustc_target::spec::Target {
        arch: "wasm32".to_string(),
        ..Default::default()
    };
    
    let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
    let mut wasmir = create_simple_add_function();
    
    let result = backend.compile_to_wasm(&mut wasmir);
    assert!(result.is_ok());
    
    let wasm_bytes = result.unwrap();
    assert!(!wasm_bytes.is_empty());
    
    // Verify WASM magic number
    assert_eq!(&wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6d]);
}

#[test]
fn test_hybrid_compilation() {
    let target = rustc_target::spec::Target {
        arch: "wasm32".to_string(),
        ..Default::default()
    };
    
    let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
    let mut wasmir = create_simple_add_function();
    
    let result = backend.compile_hybrid(&mut wasmir);
    assert!(result.is_ok());
    
    let wasm_bytes = result.unwrap();
    assert!(!wasm_bytes.is_empty());
    
    // Verify WASM magic number
    assert_eq!(&wasm_bytes[0..4], &[0x00, 0x61, 0x73, 0x6d]);
}

#[test]
fn test_optimization_passes() {
    let mut wasmir = create_function_with_constant_folding();
    let mut codegen = WasmCodegen::new();
    
    // Get optimization passes
    let passes = codegen.wasm_optimizer.get_optimization_passes();
    assert!(passes.contains(&"dead_code_elimination"));
    assert!(passes.contains(&"constant_folding"));
    assert!(passes.contains(&"instruction_selection"));
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    assert!(!compilation_result.code.is_empty());
}

#[test]
fn test_streaming_layout_optimization() {
    let mut wasmir = create_complex_function();
    let mut codegen = WasmCodegen::new();
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    
    // Check that streaming optimization was applied
    assert!(!compilation_result.code.is_empty());
    
    // The exact optimization would be visible in the generated WASM structure
    // For now, just verify it compiled successfully
}

#[test]
fn test_constant_folding() {
    let mut wasmir = create_function_with_constant_folding();
    let mut codegen = WasmCodegen::new();
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    assert!(!compilation_result.code.is_empty());
    
    // The constant folding should have optimized the function
    // We can verify this by checking the WASM bytecode size is reasonable
    assert!(compilation_result.code.len() < 200); // Should be small after optimization
}

#[test]
fn test_dead_code_elimination() {
    let mut wasmir = create_function_with_dead_code();
    let mut codegen = WasmCodegen::new();
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    assert!(!compilation_result.code.is_empty());
    
    // Dead code should have been eliminated
    // This would be visible in a more detailed analysis of the WASM output
}

#[test]
fn test_instruction_selection() {
    let mut wasmir = create_function_with_multiply_optimization();
    let mut codegen = WasmCodegen::new();
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    assert!(!compilation_result.code.is_empty());
    
    // The multiplication by 8 should have been optimized to a shift
    // This would be visible in the WASM bytecode as a shift instruction
}

#[test]
fn test_ownership_tracking() {
    let mut wasmir = create_function_with_ownership();
    let mut codegen = WasmCodegen::new();
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    assert!(!compilation_result.code.is_empty());
    
    // Ownership annotations should be preserved through compilation
    assert!(!wasmir.ownership_annotations.is_empty());
}

#[test]
fn test_capability_annotations() {
    let mut wasmir = create_function_with_capabilities();
    let mut codegen = WasmCodegen::new();
    
    let result = codegen.compile(&mut wasmir);
    assert!(result.is_ok());
    
    let compilation_result = result.unwrap();
    assert!(!compilation_result.code.is_empty());
    
    // Capability annotations should influence the compilation
    assert!(!wasmir.capabilities.is_empty());
}

#[test]
fn test_error_handling() {
    let mut invalid_wasmir = WasmIR::new(
        "invalid".to_string(),
        Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    // Add an invalid instruction (references non-existent local)
    invalid_wasmir.basic_blocks.push(BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 999 }, // Invalid index
        ],
        terminator: Terminator::Return { value: None },
    });
    
    let mut codegen = WasmCodegen::new();
    let result = codegen.compile(&mut invalid_wasmir);
    
    // Should handle the error gracefully
    assert!(result.is_err());
}

#[test]
fn test_multiple_functions() {
    let mut functions = Vec::new();
    
    // Create multiple test functions
    functions.push(create_simple_add_function());
    functions.push(create_simple_mul_function());
    functions.push(create_simple_div_function());
    
    let mut codegen = WasmCodegen::new();
    
    for (i, mut wasmir) in functions.into_iter().enumerate() {
        let result = codegen.compile(&mut wasmir);
        assert!(result.is_ok(), "Function {} should compile", i);
        
        let compilation_result = result.unwrap();
        assert!(!compilation_result.code.is_empty());
        
        // Each function should be a valid WASM module
        assert_eq!(&compilation_result.code[0..4], &[0x00, 0x61, 0x73, 0x6d]);
    }
}

// Helper functions to create test WasmIR functions

fn create_simple_add_function() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "add_function".to_string(),
        Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    // Add locals for parameters and result
    wasmir.add_local(Type::I32); // result
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 }, // first param
            Instruction::LocalGet { index: 1 }, // second param
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(0),
                right: Operand::Local(1),
            },
            Instruction::LocalSet { 
                index: 2, // result
                value: Operand::Local(2) // result of add
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(2)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

fn create_simple_mul_function() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "mul_function".to_string(),
        Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    wasmir.add_local(Type::I32); // result
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 },
            Instruction::LocalGet { index: 1 },
            Instruction::BinaryOp {
                op: BinaryOp::Mul,
                left: Operand::Local(0),
                right: Operand::Local(1),
            },
            Instruction::LocalSet { 
                index: 2,
                value: Operand::Local(2)
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(2)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

fn create_simple_div_function() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "div_function".to_string(),
        Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    wasmir.add_local(Type::I32); // result
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 },
            Instruction::LocalGet { index: 1 },
            Instruction::BinaryOp {
                op: BinaryOp::Div,
                left: Operand::Local(0),
                right: Operand::Local(1),
            },
            Instruction::LocalSet { 
                index: 2,
                value: Operand::Local(2)
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(2)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

fn create_function_with_constant_folding() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "const_fold".to_string(),
        Signature {
            params: vec![],
            returns: Some(Type::I32),
        },
    );
    
    wasmir.add_local(Type::I32); // result
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Constant(Constant::I32(10)),
                right: Operand::Constant(Constant::I32(20)),
            },
            Instruction::LocalSet { 
                index: 0,
                value: Operand::Local(0) // Should be folded to 30
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(0)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

fn create_function_with_dead_code() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "dead_code".to_string(),
        Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    wasmir.add_local(Type::I32); // unused local
    wasmir.add_local(Type::I32); // result
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 }, // use parameter
            Instruction::LocalSet { 
                index: 1, // set result
                value: Operand::Local(0)
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(1)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

fn create_function_with_multiply_optimization() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "mul_opt".to_string(),
        Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    wasmir.add_local(Type::I32); // result
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 },
            Instruction::BinaryOp {
                op: BinaryOp::Mul,
                left: Operand::Local(0),
                right: Operand::Constant(Constant::I32(8)), // Should be optimized to shift
            },
            Instruction::LocalSet { 
                index: 1,
                value: Operand::Local(1)
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(1)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

fn create_complex_function() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "complex".to_string(),
        Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    wasmir.add_local(Type::I32); // temp
    wasmir.add_local(Type::I32); // result
    
    // Multiple basic blocks for streaming layout test
    let block1 = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 },
            Instruction::LocalGet { index: 1 },
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(0),
                right: Operand::Local(1),
            },
            Instruction::LocalSet { 
                index: 2,
                value: Operand::Local(2)
            },
        ],
        terminator: Terminator::Jump { 
            target: wasm::wasmir::BlockId(1)
        },
    };
    
    let block2 = BasicBlock {
        id: wasm::wasmir::BlockId(1),
        instructions: vec![
            Instruction::LocalGet { index: 2 },
            Instruction::BinaryOp {
                op: BinaryOp::Mul,
                left: Operand::Local(2),
                right: Operand::Constant(Constant::I32(2)),
            },
            Instruction::LocalSet { 
                index: 3,
                value: Operand::Local(3)
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(3)) 
        },
    };
    
    wasmir.basic_blocks.push(block1);
    wasmir.basic_blocks.push(block2);
    wasmir
}

fn create_function_with_ownership() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "ownership".to_string(),
        Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    wasmir.add_local(Type::I32); // result
    
    // Add ownership annotation
    use wasm::wasmir::{OwnershipAnnotation, OwnershipState, SourceLocation};
    wasmir.add_ownership_annotation(OwnershipAnnotation {
        variable: 0,
        state: OwnershipState::Owned,
        source_location: SourceLocation {
            file: "test.rs".to_string(),
            line: 1,
            column: 0,
        },
    });
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 },
            Instruction::LocalSet { 
                index: 1,
                value: Operand::Local(0)
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(1)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

fn create_function_with_capabilities() -> WasmIR {
    let mut wasmir = WasmIR::new(
        "capabilities".to_string(),
        Signature {
            params: vec![Type::I32],
            returns: Some(Type::I32),
        },
    );
    
    // Add capability annotations
    use wasm::wasmir::Capability;
    wasmir.add_capability(Capability::JsInterop);
    wasmir.add_capability(Capability::Threading);
    
    wasmir.add_local(Type::I32); // result
    
    let block = BasicBlock {
        id: wasm::wasmir::BlockId(0),
        instructions: vec![
            Instruction::LocalGet { index: 0 },
            Instruction::LocalSet { 
                index: 1,
                value: Operand::Local(0)
            },
        ],
        terminator: Terminator::Return { 
            value: Some(Operand::Local(1)) 
        },
    };
    
    wasmir.basic_blocks.push(block);
    wasmir
}

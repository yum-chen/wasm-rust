//! Executable Unit Tests for the WasmIR Data Structure
//!
//! These tests are self-contained and can be run with `cargo test`. They
//! verify the internal consistency and validation logic of the WasmIR struct.

use super::*;

#[test]
fn test_validate_valid_function() {
    let signature = Signature {
        params: vec![Type::I32],
        returns: Some(Type::I32),
    };
    let mut func = WasmIR::new("test".to_string(), signature);

    let local_index = func.add_local(Type::I32);
    let instructions = vec![Instruction::LocalGet { index: local_index }];
    let terminator = Terminator::Return { value: Some(Operand::Local(local_index)) };

    func.add_basic_block(instructions, terminator);

    assert!(func.validate().is_ok(), "Validation should pass for a well-formed function.");
}

#[test]
fn test_validate_invalid_local_index_in_instruction() {
    let signature = Signature {
        params: vec![],
        returns: None,
    };
    let mut func = WasmIR::new("test".to_string(), signature);

    let instructions = vec![Instruction::LocalGet { index: 99 }]; // Invalid index
    let terminator = Terminator::Return { value: None };

    func.add_basic_block(instructions, terminator);

    let result = func.validate();
    assert!(result.is_err());
    match result.unwrap_err() {
        ValidationError::InvalidLocalIndex(idx) => assert_eq!(idx, 99),
        _ => panic!("Expected an InvalidLocalIndex error."),
    }
}

#[test]
fn test_validate_invalid_local_index_in_terminator() {
    let signature = Signature {
        params: vec![],
        returns: Some(Type::I32),
    };
    let mut func = WasmIR::new("test".to_string(), signature);

    let instructions = vec![];
    let terminator = Terminator::Return { value: Some(Operand::Local(123)) }; // Invalid index

    func.add_basic_block(instructions, terminator);

    let result = func.validate();
    assert!(result.is_err());
    match result.unwrap_err() {
        ValidationError::InvalidLocalIndex(idx) => assert_eq!(idx, 123),
        _ => panic!("Expected an InvalidLocalIndex error."),
    }
}

#[test]
fn test_validate_invalid_block_id_in_terminator() {
    let signature = Signature {
        params: vec![],
        returns: None,
    };
    let mut func = WasmIR::new("test".to_string(), signature);

    let instructions = vec![];
    // This terminator jumps to a block that doesn't exist.
    let terminator = Terminator::Jump { target: BlockId(1) };

    func.add_basic_block(instructions, terminator);

    let result = func.validate();
    assert!(result.is_err());
    match result.unwrap_err() {
        ValidationError::InvalidBlockId(desc) => assert_eq!(desc, "jump_target"),
        _ => panic!("Expected an InvalidBlockId error."),
    }
}

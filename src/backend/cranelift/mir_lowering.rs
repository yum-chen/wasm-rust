//! MIR to WasmIR lowering for WasmRust
//! 
//! This module implements the transformation from Rust MIR to WasmIR,
//! preserving Rust semantics while enabling WASM-specific optimizations.
//! 
//! The lowering process follows these key principles:
//! 1. Preserve Rust's ownership and borrowing semantics
//! 2. Add WASM-specific optimization annotations
//! 3. Maintain debug information for source-level debugging
//! 4. Enforce the Compiler-Crate Contract for safe optimizations

use wasm::wasmir::{
    WasmIR, Instruction, Terminator, BasicBlock, BlockId, Type, Signature, Operand, 
    BinaryOp, UnaryOp, OwnershipState, OwnershipAnnotation, SourceLocation, Capability,
    Constant, AtomicOp, LinearOp, MemoryOrder, ValidationError
};
use std::collections::{HashMap, HashSet};

/// Simulated Rust MIR types for demonstration
/// In a real implementation, these would come from rustc_middle::mir
#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub signature: MirSignature,
    pub basic_blocks: Vec<MirBasicBlock>,
    pub local_decls: Vec<MirLocalDecl>,
    pub source_info: MirSourceInfo,
}

#[derive(Debug, Clone)]
pub struct MirSignature {
    pub inputs: Vec<MirType>,
    pub output: MirType,
}

#[derive(Debug, Clone)]
pub struct MirBasicBlock {
    pub statements: Vec<MirStatement>,
    pub terminator: MirTerminator,
}

#[derive(Debug, Clone)]
pub struct MirLocalDecl {
    pub ty: MirType,
    pub source_info: MirSourceInfo,
}

#[derive(Debug, Clone)]
pub struct MirSourceInfo {
    pub span: MirSpan,
}

#[derive(Debug, Clone)]
pub struct MirSpan {
    pub filename: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
pub enum MirType {
    I32,
    I64,
    F32,
    F64,
    Bool,
    Ref(Box<MirType>),
    ExternRef(String),
    FuncRef,
    Array(Box<MirType>, u32),
    Struct(Vec<MirType>),
    Unit,
}

#[derive(Debug, Clone)]
pub enum MirStatement {
    Assign(MirPlace, MirRvalue),
    StorageLive(u32),
    StorageDead(u32),
    Nop,
}

#[derive(Debug, Clone)]
pub enum MirRvalue {
    Use(MirOperand),
    BinaryOp(MirBinOp, MirOperand, MirOperand),
    UnaryOp(MirUnOp, MirOperand),
    Cast(MirOperand, MirType),
    Ref(MirOperand),
    Len(MirOperand),
}

#[derive(Debug, Clone)]
pub enum MirTerminator {
    Return,
    Goto { target: u32 },
    SwitchInt { discr: MirOperand, targets: Vec<(i32, u32)>, otherwise: u32 },
    Call { func: MirOperand, args: Vec<MirOperand>, destination: Option<(MirPlace, u32)> },
    Unreachable,
}

#[derive(Debug, Clone)]
pub enum MirPlace {
    Local(u32),
    Projection(Box<MirPlace>, Box<MirProjection>),
}

#[derive(Debug, Clone)]
pub enum MirProjection {
    Deref,
    Field(u32),
    Index(Box<MirOperand>),
}

#[derive(Debug, Clone)]
pub enum MirOperand {
    Copy(Box<MirPlace>),
    Move(Box<MirPlace>),
    Constant(MirConstant),
}

#[derive(Debug, Clone)]
pub enum MirConstant {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Unit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirBinOp {
    Add, Sub, Mul, Div, Rem,
    BitXor, BitAnd, BitOr,
    Shl, Shr,
    Eq, Lt, Le, Ne, Ge, Gt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirUnOp {
    Not, Neg,
}

/// Context for lowering MIR to WasmIR
pub struct MirLoweringContext {
    /// Current function being lowered
    current_function: Option<WasmIR>,
    /// Local variable mappings from MIR local index to WasmIR local index
    local_mappings: HashMap<u32, u32>,
    /// Basic block mappings from MIR block index to WasmIR BlockId
    block_mappings: HashMap<u32, BlockId>,
    /// Error messages collected during lowering
    error_messages: Vec<String>,
    /// Debug information preservation
    debug_info: HashMap<u32, SourceLocation>,
    /// Ownership tracking for linear types
    ownership_tracker: OwnershipTracker,
    /// Capability requirements detected during lowering
    required_capabilities: HashSet<Capability>,
}

/// Tracks ownership states for linear types during MIR lowering
#[derive(Debug, Default)]
pub struct OwnershipTracker {
    /// Current ownership state of each local variable
    local_states: HashMap<u32, OwnershipState>,
    /// Ownership annotations to be added to WasmIR
    annotations: Vec<OwnershipAnnotation>,
}

impl OwnershipTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the ownership state of a local variable
    pub fn set_ownership(&mut self, local: u32, state: OwnershipState, location: SourceLocation) {
        self.local_states.insert(local, state);
        self.annotations.push(OwnershipAnnotation {
            variable: local,
            state,
            source_location: location,
        });
    }

    /// Gets the current ownership state of a local variable
    pub fn get_ownership(&self, local: u32) -> Option<OwnershipState> {
        self.local_states.get(&local).copied()
    }

    /// Consumes the tracker and returns the collected annotations
    pub fn into_annotations(self) -> Vec<OwnershipAnnotation> {
        self.annotations
    }
}

impl MirLoweringContext {
    /// Creates a new MIR lowering context
    pub fn new() -> Self {
        Self {
            current_function: None,
            local_mappings: HashMap::new(),
            block_mappings: HashMap::new(),
            error_messages: Vec::new(),
            debug_info: HashMap::new(),
            ownership_tracker: OwnershipTracker::new(),
            required_capabilities: HashSet::new(),
        }
    }

    /// Main entry point for lowering a MIR function to WasmIR
    pub fn lower_function(&mut self, mir_func: &MirFunction) -> Result<WasmIR, String> {
        // Convert MIR signature to WasmIR signature
        let signature = self.convert_signature(&mir_func.signature)?;
        
        // Create new WasmIR function
        let mut wasmir_func = WasmIR::new(mir_func.name.clone(), signature);
        
        // Add local variables
        for (index, local_decl) in mir_func.local_decls.iter().enumerate() {
            let wasmir_type = self.convert_type(&local_decl.ty)?;
            let local_index = wasmir_func.add_local(wasmir_type);
            self.local_mappings.insert(index as u32, local_index);
            
            // Preserve debug information
            let source_location = SourceLocation {
                file: local_decl.source_info.span.filename.clone(),
                line: local_decl.source_info.span.line,
                column: local_decl.source_info.span.column,
            };
            self.debug_info.insert(local_index, source_location.clone());
            
            // Initialize ownership tracking for linear types
            if self.is_linear_type(&local_decl.ty) {
                self.ownership_tracker.set_ownership(local_index, OwnershipState::Owned, source_location);
            }
        }
        
        // Create block mappings
        for (index, _) in mir_func.basic_blocks.iter().enumerate() {
            let block_id = BlockId(index);
            self.block_mappings.insert(index as u32, block_id);
        }
        
        // Convert basic blocks
        for (bb_index, mir_bb) in mir_func.basic_blocks.iter().enumerate() {
            let instructions = self.convert_statements(&mir_bb.statements)?;
            let terminator = self.convert_terminator(&mir_bb.terminator)?;
            wasmir_func.add_basic_block(instructions, terminator);
        }
        
        // Add capability annotations
        for capability in &self.required_capabilities {
            wasmir_func.add_capability(capability.clone());
        }
        
        // Add ownership annotations
        let ownership_annotations = std::mem::take(&mut self.ownership_tracker).into_annotations();
        for annotation in ownership_annotations {
            wasmir_func.add_ownership_annotation(annotation);
        }
        
        // Validate the generated WasmIR
        wasmir_func.validate().map_err(|e| format!("WasmIR validation failed: {}", e))?;
        
        Ok(wasmir_func)
    }

    /// Converts MIR signature to WasmIR signature
    fn convert_signature(&self, mir_sig: &MirSignature) -> Result<Signature, String> {
        let mut params = Vec::new();
        for input_ty in &mir_sig.inputs {
            params.push(self.convert_type(input_ty)?);
        }
        
        let returns = match mir_sig.output {
            MirType::Unit => None,
            _ => Some(self.convert_type(&mir_sig.output)?),
        };
        
        Ok(Signature { params, returns })
    }

    /// Converts MIR type to WasmIR type
    fn convert_type(&self, mir_ty: &MirType) -> Result<Type, String> {
        match mir_ty {
            MirType::I32 => Ok(Type::I32),
            MirType::I64 => Ok(Type::I64),
            MirType::F32 => Ok(Type::F32),
            MirType::F64 => Ok(Type::F64),
            MirType::Bool => Ok(Type::I32), // Booleans are represented as i32 in WASM
            MirType::ExternRef(type_name) => Ok(Type::ExternRef(type_name.clone())),
            MirType::FuncRef => Ok(Type::FuncRef),
            MirType::Ref(inner_ty) => {
                // References become pointers in WASM
                Ok(Type::Pointer(Box::new(self.convert_type(inner_ty)?)))
            }
            MirType::Array(element_ty, size) => {
                Ok(Type::Array {
                    element_type: Box::new(self.convert_type(element_ty)?),
                    size: Some(*size),
                })
            }
            MirType::Struct(field_types) => {
                let mut fields = Vec::new();
                for field_ty in field_types {
                    fields.push(self.convert_type(field_ty)?);
                }
                Ok(Type::Struct { fields })
            }
            MirType::Unit => Ok(Type::Void),
        }
    }

    /// Checks if a MIR type should be treated as a linear type
    fn is_linear_type(&mut self, mir_ty: &MirType) -> bool {
        match mir_ty {
            MirType::ExternRef(_) => {
                // ExternRef requires JS interop capability
                self.required_capabilities.insert(Capability::JsInterop);
                true
            },
            MirType::FuncRef => true,      // FuncRef has linear semantics
            _ => false,
        }
    }

    /// Converts MIR statements to WasmIR instructions
    fn convert_statements(&mut self, statements: &[MirStatement]) -> Result<Vec<Instruction>, String> {
        let mut instructions = Vec::new();
        
        for statement in statements {
            match statement {
                MirStatement::Assign(place, rvalue) => {
                    let wasmir_instructions = self.convert_assignment(place, rvalue)?;
                    instructions.extend(wasmir_instructions);
                }
                MirStatement::StorageLive(local) => {
                    // Storage live/dead are handled implicitly in WasmIR
                    // But we can use this for ownership tracking
                    if let Some(&wasmir_local) = self.local_mappings.get(local) {
                        if let Some(debug_info) = self.debug_info.get(&wasmir_local).cloned() {
                            self.ownership_tracker.set_ownership(wasmir_local, OwnershipState::Owned, debug_info);
                        }
                    }
                }
                MirStatement::StorageDead(local) => {
                    // Mark as consumed for linear types
                    if let Some(&wasmir_local) = self.local_mappings.get(local) {
                        if let Some(debug_info) = self.debug_info.get(&wasmir_local).cloned() {
                            self.ownership_tracker.set_ownership(wasmir_local, OwnershipState::Consumed, debug_info);
                        }
                    }
                }
                MirStatement::Nop => {
                    instructions.push(Instruction::Nop);
                }
            }
        }
        
        Ok(instructions)
    }

    /// Converts a MIR assignment to WasmIR instructions
    fn convert_assignment(&mut self, place: &MirPlace, rvalue: &MirRvalue) -> Result<Vec<Instruction>, String> {
        let mut instructions = Vec::new();
        
        match rvalue {
            MirRvalue::Use(operand) => {
                let wasmir_operand = self.convert_operand(operand)?;
                let place_local = self.convert_place_to_local(place)?;
                
                // Handle ownership transfer for linear types
                if let MirOperand::Move(moved_place) = operand {
                    if let Ok(moved_local) = self.convert_place_to_local(moved_place.as_ref()) {
                        if let Some(debug_info) = self.debug_info.get(&moved_local).cloned() {
                            self.ownership_tracker.set_ownership(moved_local, OwnershipState::Moved, debug_info.clone());
                            self.ownership_tracker.set_ownership(place_local, OwnershipState::Owned, debug_info);
                        }
                    }
                }
                
                instructions.push(Instruction::LocalSet {
                    index: place_local,
                    value: wasmir_operand,
                });
            }
            MirRvalue::BinaryOp(op, left, right) => {
                let wasmir_op = self.convert_binary_op(*op)?;
                let left_operand = self.convert_operand(left)?;
                let right_operand = self.convert_operand(right)?;
                let place_local = self.convert_place_to_local(place)?;
                
                instructions.push(Instruction::BinaryOp {
                    op: wasmir_op,
                    left: left_operand,
                    right: right_operand,
                });
                
                // Store result in place
                instructions.push(Instruction::LocalSet {
                    index: place_local,
                    value: Operand::StackValue(0), // Result of binary op
                });
            }
            MirRvalue::UnaryOp(op, operand) => {
                let wasmir_op = self.convert_unary_op(*op)?;
                let wasmir_operand = self.convert_operand(operand)?;
                let place_local = self.convert_place_to_local(place)?;
                
                instructions.push(Instruction::UnaryOp {
                    op: wasmir_op,
                    value: wasmir_operand,
                });
                
                instructions.push(Instruction::LocalSet {
                    index: place_local,
                    value: Operand::StackValue(0),
                });
            }
            MirRvalue::Cast(operand, target_ty) => {
                let wasmir_operand = self.convert_operand(operand)?;
                let target_type = self.convert_type(target_ty)?;
                let place_local = self.convert_place_to_local(place)?;
                
                // Handle ExternRef casts specially
                if let Type::ExternRef(_) = target_type {
                    self.required_capabilities.insert(Capability::JsInterop);
                    instructions.push(Instruction::ExternRefCast {
                        externref: wasmir_operand,
                        target_type,
                    });
                } else {
                    // For now, treat other casts as no-ops or simple moves
                    instructions.push(Instruction::LocalSet {
                        index: place_local,
                        value: wasmir_operand,
                    });
                }
            }
            MirRvalue::Ref(operand) => {
                // Taking a reference - this becomes a pointer in WASM
                let wasmir_operand = self.convert_operand(operand)?;
                let place_local = self.convert_place_to_local(place)?;
                
                instructions.push(Instruction::LocalSet {
                    index: place_local,
                    value: wasmir_operand,
                });
            }
            MirRvalue::Len(operand) => {
                // Array/slice length operation
                let wasmir_operand = self.convert_operand(operand)?;
                let place_local = self.convert_place_to_local(place)?;
                
                // For now, assume length is stored as part of the slice structure
                instructions.push(Instruction::LocalSet {
                    index: place_local,
                    value: wasmir_operand,
                });
            }
        }
        
        Ok(instructions)
    }

    /// Converts MIR terminator to WasmIR terminator
    fn convert_terminator(&mut self, terminator: &MirTerminator) -> Result<Terminator, String> {
        match terminator {
            MirTerminator::Return => {
                Ok(Terminator::Return { value: None })
            }
            MirTerminator::Goto { target } => {
                let target_block = self.block_mappings.get(target)
                    .ok_or_else(|| format!("Invalid block target: {}", target))?;
                Ok(Terminator::Jump { target: *target_block })
            }
            MirTerminator::SwitchInt { discr, targets, otherwise } => {
                let condition = self.convert_operand(discr)?;
                let mut wasmir_targets = Vec::new();
                
                for (value, target) in targets {
                    let target_block = self.block_mappings.get(target)
                        .ok_or_else(|| format!("Invalid switch target: {}", target))?;
                    wasmir_targets.push((Operand::Constant(Constant::I32(*value)), *target_block));
                }
                
                let default_target = self.block_mappings.get(otherwise)
                    .ok_or_else(|| format!("Invalid default target: {}", otherwise))?;
                
                Ok(Terminator::Switch {
                    value: condition,
                    targets: wasmir_targets,
                    default_target: *default_target,
                })
            }
            MirTerminator::Call { func, args, destination } => {
                // For now, convert calls to a simplified form
                // In a real implementation, this would handle function resolution
                let _func_operand = self.convert_operand(func)?;
                let mut wasmir_args = Vec::new();
                
                for arg in args {
                    wasmir_args.push(self.convert_operand(arg)?);
                }
                
                if let Some((dest_place, target)) = destination {
                    let _dest_local = self.convert_place_to_local(dest_place)?;
                    let target_block = self.block_mappings.get(target)
                        .ok_or_else(|| format!("Invalid call target: {}", target))?;
                    
                    // For now, just jump to the target block
                    Ok(Terminator::Jump { target: *target_block })
                } else {
                    Ok(Terminator::Unreachable)
                }
            }
            MirTerminator::Unreachable => {
                Ok(Terminator::Unreachable)
            }
        }
    }

    /// Converts MIR operand to WasmIR operand
    fn convert_operand(&mut self, operand: &MirOperand) -> Result<Operand, String> {
        match operand {
            MirOperand::Copy(place) => {
                let local = self.convert_place_to_local(place.as_ref())?;
                Ok(Operand::Local(local))
            }
            MirOperand::Move(place) => {
                let local = self.convert_place_to_local(place.as_ref())?;
                
                // Track ownership transfer for linear types
                if let Some(debug_info) = self.debug_info.get(&local).cloned() {
                    self.ownership_tracker.set_ownership(local, OwnershipState::Moved, debug_info);
                }
                
                Ok(Operand::Local(local))
            }
            MirOperand::Constant(constant) => {
                let wasmir_constant = self.convert_constant(constant)?;
                Ok(Operand::Constant(wasmir_constant))
            }
        }
    }

    /// Converts MIR place to WasmIR local index
    fn convert_place_to_local(&self, place: &MirPlace) -> Result<u32, String> {
        match place {
            MirPlace::Local(local) => {
                self.local_mappings.get(local)
                    .copied()
                    .ok_or_else(|| format!("Unknown local: {}", local))
            }
            MirPlace::Projection(base, projection) => {
                // For now, handle simple projections
                match projection.as_ref() {
                    MirProjection::Deref => {
                        // Dereference - for now, just return the base
                        self.convert_place_to_local(base)
                    }
                    MirProjection::Field(_field) => {
                        // Field access - for now, just return the base
                        self.convert_place_to_local(base)
                    }
                    MirProjection::Index(_index) => {
                        // Array index - for now, just return the base
                        self.convert_place_to_local(base)
                    }
                }
            }
        }
    }

    /// Converts MIR constant to WasmIR constant
    fn convert_constant(&self, constant: &MirConstant) -> Result<Constant, String> {
        match constant {
            MirConstant::I32(value) => Ok(Constant::I32(*value)),
            MirConstant::I64(value) => Ok(Constant::I64(*value)),
            MirConstant::F32(value) => Ok(Constant::F32(*value)),
            MirConstant::F64(value) => Ok(Constant::F64(*value)),
            MirConstant::Bool(value) => Ok(Constant::Boolean(*value)),
            MirConstant::Unit => Ok(Constant::I32(0)), // Unit represented as 0
        }
    }

    /// Converts MIR binary operation to WasmIR binary operation
    fn convert_binary_op(&self, op: MirBinOp) -> Result<BinaryOp, String> {
        match op {
            MirBinOp::Add => Ok(BinaryOp::Add),
            MirBinOp::Sub => Ok(BinaryOp::Sub),
            MirBinOp::Mul => Ok(BinaryOp::Mul),
            MirBinOp::Div => Ok(BinaryOp::Div),
            MirBinOp::Rem => Ok(BinaryOp::Mod),
            MirBinOp::BitXor => Ok(BinaryOp::Xor),
            MirBinOp::BitAnd => Ok(BinaryOp::And),
            MirBinOp::BitOr => Ok(BinaryOp::Or),
            MirBinOp::Shl => Ok(BinaryOp::Shl),
            MirBinOp::Shr => Ok(BinaryOp::Shr),
            MirBinOp::Eq => Ok(BinaryOp::Eq),
            MirBinOp::Lt => Ok(BinaryOp::Lt),
            MirBinOp::Le => Ok(BinaryOp::Le),
            MirBinOp::Ne => Ok(BinaryOp::Ne),
            MirBinOp::Ge => Ok(BinaryOp::Ge),
            MirBinOp::Gt => Ok(BinaryOp::Gt),
        }
    }

    /// Converts MIR unary operation to WasmIR unary operation
    fn convert_unary_op(&self, op: MirUnOp) -> Result<UnaryOp, String> {
        match op {
            MirUnOp::Not => Ok(UnaryOp::Not),
            MirUnOp::Neg => Ok(UnaryOp::Neg),
        }
    }

    /// Creates a simple WasmIR function for testing
    pub fn create_simple_function(&mut self, name: String) -> WasmIR {
        let signature = Signature {
            params: vec![Type::I32, Type::I32],
            returns: Some(Type::I32),
        };
        
        let mut wasm_func = WasmIR::new(name, signature);
        
        // Add some locals
        wasm_func.add_local(Type::I32); // Result local
        
        // Create a simple basic block that adds two parameters
        let instructions = vec![
            Instruction::BinaryOp {
                op: BinaryOp::Add,
                left: Operand::Local(0),  // First parameter
                right: Operand::Local(1), // Second parameter
            },
        ];
        
        let terminator = Terminator::Return {
            value: Some(Operand::Local(2)), // Return the result
        };
        
        wasm_func.add_basic_block(instructions, terminator);
        
        wasm_func
    }

    /// Checks if there are any errors
    pub fn has_errors(&self) -> bool {
        !self.error_messages.is_empty()
    }

    /// Gets error messages
    pub fn get_errors(&self) -> &[String] {
        &self.error_messages
    }

    /// Converts the context into a WasmIR function
    pub fn into_wasmir(self) -> Result<WasmIR, String> {
        if let Some(func) = self.current_function {
            Ok(func)
        } else {
            // Create a default function for testing
            let signature = Signature {
                params: vec![Type::I32, Type::I32],
                returns: Some(Type::I32),
            };
            
            let mut wasm_func = WasmIR::new("default_function".to_string(), signature);
            
            // Add a simple basic block
            let instructions = vec![
                Instruction::BinaryOp {
                    op: BinaryOp::Add,
                    left: Operand::Local(0),
                    right: Operand::Local(1),
                },
            ];
            
            let terminator = Terminator::Return {
                value: Some(Operand::Local(0)),
            };
            
            wasm_func.add_basic_block(instructions, terminator);
            
            Ok(wasm_func)
        }
    }

    /// Adds an error message
    pub fn add_error(&mut self, message: String) {
        self.error_messages.push(message);
    }

    /// Sets the current function
    pub fn set_function(&mut self, func: WasmIR) {
        self.current_function = Some(func);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mir_lowering_context_creation() {
        let context = MirLoweringContext::new();
        assert!(!context.has_errors());
        assert_eq!(context.get_errors().len(), 0);
        assert_eq!(context.local_mappings.len(), 0);
        assert_eq!(context.block_mappings.len(), 0);
    }

    #[test]
    fn test_simple_function_creation() {
        let mut context = MirLoweringContext::new();
        let func = context.create_simple_function("test_func".to_string());
        
        assert_eq!(func.name, "test_func");
        assert_eq!(func.signature.params.len(), 2);
        assert_eq!(func.signature.returns, Some(Type::I32));
        assert_eq!(func.basic_blocks.len(), 1);
    }

    #[test]
    fn test_into_wasmir() {
        let context = MirLoweringContext::new();
        let result = context.into_wasmir();
        
        assert!(result.is_ok());
        let func = result.unwrap();
        assert_eq!(func.name, "default_function");
        assert_eq!(func.basic_blocks.len(), 1);
    }

    #[test]
    fn test_error_handling() {
        let mut context = MirLoweringContext::new();
        assert!(!context.has_errors());
        
        context.add_error("Test error".to_string());
        assert!(context.has_errors());
        assert_eq!(context.get_errors().len(), 1);
        assert_eq!(context.get_errors()[0], "Test error");
    }

    #[test]
    fn test_type_conversion() {
        let context = MirLoweringContext::new();
        
        // Test basic type conversions
        assert_eq!(context.convert_type(&MirType::I32).unwrap(), Type::I32);
        assert_eq!(context.convert_type(&MirType::I64).unwrap(), Type::I64);
        assert_eq!(context.convert_type(&MirType::F32).unwrap(), Type::F32);
        assert_eq!(context.convert_type(&MirType::F64).unwrap(), Type::F64);
        assert_eq!(context.convert_type(&MirType::Bool).unwrap(), Type::I32);
        assert_eq!(context.convert_type(&MirType::Unit).unwrap(), Type::Void);
        
        // Test ExternRef conversion
        let externref_type = context.convert_type(&MirType::ExternRef("JsObject".to_string())).unwrap();
        assert_eq!(externref_type, Type::ExternRef("JsObject".to_string()));
        
        // Test FuncRef conversion
        assert_eq!(context.convert_type(&MirType::FuncRef).unwrap(), Type::FuncRef);
    }

    #[test]
    fn test_signature_conversion() {
        let context = MirLoweringContext::new();
        
        let mir_sig = MirSignature {
            inputs: vec![MirType::I32, MirType::F32],
            output: MirType::I64,
        };
        
        let wasmir_sig = context.convert_signature(&mir_sig).unwrap();
        assert_eq!(wasmir_sig.params.len(), 2);
        assert_eq!(wasmir_sig.params[0], Type::I32);
        assert_eq!(wasmir_sig.params[1], Type::F32);
        assert_eq!(wasmir_sig.returns, Some(Type::I64));
    }

    #[test]
    fn test_constant_conversion() {
        let context = MirLoweringContext::new();
        
        assert_eq!(context.convert_constant(&MirConstant::I32(42)).unwrap(), Constant::I32(42));
        assert_eq!(context.convert_constant(&MirConstant::I64(123)).unwrap(), Constant::I64(123));
        assert_eq!(context.convert_constant(&MirConstant::F32(3.14)).unwrap(), Constant::F32(3.14));
        assert_eq!(context.convert_constant(&MirConstant::F64(2.71)).unwrap(), Constant::F64(2.71));
        assert_eq!(context.convert_constant(&MirConstant::Bool(true)).unwrap(), Constant::Boolean(true));
        assert_eq!(context.convert_constant(&MirConstant::Unit).unwrap(), Constant::I32(0));
    }

    #[test]
    fn test_binary_op_conversion() {
        let context = MirLoweringContext::new();
        
        assert_eq!(context.convert_binary_op(MirBinOp::Add).unwrap(), BinaryOp::Add);
        assert_eq!(context.convert_binary_op(MirBinOp::Sub).unwrap(), BinaryOp::Sub);
        assert_eq!(context.convert_binary_op(MirBinOp::Mul).unwrap(), BinaryOp::Mul);
        assert_eq!(context.convert_binary_op(MirBinOp::Div).unwrap(), BinaryOp::Div);
        assert_eq!(context.convert_binary_op(MirBinOp::Rem).unwrap(), BinaryOp::Mod);
        assert_eq!(context.convert_binary_op(MirBinOp::BitXor).unwrap(), BinaryOp::Xor);
        assert_eq!(context.convert_binary_op(MirBinOp::BitAnd).unwrap(), BinaryOp::And);
        assert_eq!(context.convert_binary_op(MirBinOp::BitOr).unwrap(), BinaryOp::Or);
        assert_eq!(context.convert_binary_op(MirBinOp::Shl).unwrap(), BinaryOp::Shl);
        assert_eq!(context.convert_binary_op(MirBinOp::Shr).unwrap(), BinaryOp::Shr);
        assert_eq!(context.convert_binary_op(MirBinOp::Eq).unwrap(), BinaryOp::Eq);
        assert_eq!(context.convert_binary_op(MirBinOp::Lt).unwrap(), BinaryOp::Lt);
        assert_eq!(context.convert_binary_op(MirBinOp::Le).unwrap(), BinaryOp::Le);
        assert_eq!(context.convert_binary_op(MirBinOp::Ne).unwrap(), BinaryOp::Ne);
        assert_eq!(context.convert_binary_op(MirBinOp::Ge).unwrap(), BinaryOp::Ge);
        assert_eq!(context.convert_binary_op(MirBinOp::Gt).unwrap(), BinaryOp::Gt);
    }

    #[test]
    fn test_unary_op_conversion() {
        let context = MirLoweringContext::new();
        
        assert_eq!(context.convert_unary_op(MirUnOp::Not).unwrap(), UnaryOp::Not);
        assert_eq!(context.convert_unary_op(MirUnOp::Neg).unwrap(), UnaryOp::Neg);
    }

    #[test]
    fn test_linear_type_detection() {
        let mut context = MirLoweringContext::new();
        
        // Linear types
        assert!(context.is_linear_type(&MirType::ExternRef("JsObject".to_string())));
        assert!(context.is_linear_type(&MirType::FuncRef));
        
        // Non-linear types
        assert!(!context.is_linear_type(&MirType::I32));
        assert!(!context.is_linear_type(&MirType::F64));
        assert!(!context.is_linear_type(&MirType::Bool));
        assert!(!context.is_linear_type(&MirType::Unit));
    }

    #[test]
    fn test_ownership_tracker() {
        let mut tracker = OwnershipTracker::new();
        
        let location = SourceLocation {
            file: "test.rs".to_string(),
            line: 10,
            column: 5,
        };
        
        // Test setting ownership
        tracker.set_ownership(0, OwnershipState::Owned, location.clone());
        assert_eq!(tracker.get_ownership(0), Some(OwnershipState::Owned));
        
        // Test ownership transfer
        tracker.set_ownership(0, OwnershipState::Moved, location.clone());
        assert_eq!(tracker.get_ownership(0), Some(OwnershipState::Moved));
        
        // Test annotations
        let annotations = tracker.into_annotations();
        assert_eq!(annotations.len(), 2);
        assert_eq!(annotations[0].variable, 0);
        assert_eq!(annotations[0].state, OwnershipState::Owned);
        assert_eq!(annotations[1].variable, 0);
        assert_eq!(annotations[1].state, OwnershipState::Moved);
    }

    #[test]
    fn test_complete_mir_lowering() {
        let mut context = MirLoweringContext::new();
        
        // Create a simple MIR function: fn add(a: i32, b: i32) -> i32 { a + b }
        let mir_func = MirFunction {
            name: "add".to_string(),
            signature: MirSignature {
                inputs: vec![MirType::I32, MirType::I32],
                output: MirType::I32,
            },
            basic_blocks: vec![
                MirBasicBlock {
                    statements: vec![
                        MirStatement::Assign(
                            MirPlace::Local(2), // result local
                            MirRvalue::BinaryOp(
                                MirBinOp::Add,
                                MirOperand::Copy(Box::new(MirPlace::Local(0))), // first param
                                MirOperand::Copy(Box::new(MirPlace::Local(1))), // second param
                            ),
                        ),
                    ],
                    terminator: MirTerminator::Return,
                },
            ],
            local_decls: vec![
                MirLocalDecl {
                    ty: MirType::I32,
                    source_info: MirSourceInfo {
                        span: MirSpan {
                            filename: "test.rs".to_string(),
                            line: 1,
                            column: 10,
                        },
                    },
                },
                MirLocalDecl {
                    ty: MirType::I32,
                    source_info: MirSourceInfo {
                        span: MirSpan {
                            filename: "test.rs".to_string(),
                            line: 1,
                            column: 20,
                        },
                    },
                },
                MirLocalDecl {
                    ty: MirType::I32,
                    source_info: MirSourceInfo {
                        span: MirSpan {
                            filename: "test.rs".to_string(),
                            line: 1,
                            column: 30,
                        },
                    },
                },
            ],
            source_info: MirSourceInfo {
                span: MirSpan {
                    filename: "test.rs".to_string(),
                    line: 1,
                    column: 1,
                },
            },
        };
        
        // Lower the MIR function to WasmIR
        let result = context.lower_function(&mir_func);
        assert!(result.is_ok());
        
        let wasmir_func = result.unwrap();
        assert_eq!(wasmir_func.name, "add");
        assert_eq!(wasmir_func.signature.params.len(), 2);
        assert_eq!(wasmir_func.signature.returns, Some(Type::I32));
        assert_eq!(wasmir_func.basic_blocks.len(), 1);
        assert_eq!(wasmir_func.locals.len(), 3);
        
        // Validate the function
        assert!(wasmir_func.validate().is_ok());
    }

    #[test]
    fn test_mir_lowering_with_linear_types() {
        let mut context = MirLoweringContext::new();
        
        // Create a MIR function with ExternRef (linear type)
        let mir_func = MirFunction {
            name: "use_externref".to_string(),
            signature: MirSignature {
                inputs: vec![MirType::ExternRef("JsObject".to_string())],
                output: MirType::Unit,
            },
            basic_blocks: vec![
                MirBasicBlock {
                    statements: vec![
                        MirStatement::Assign(
                            MirPlace::Local(1), // temp local
                            MirRvalue::Use(MirOperand::Move(Box::new(MirPlace::Local(0)))), // move the ExternRef
                        ),
                    ],
                    terminator: MirTerminator::Return,
                },
            ],
            local_decls: vec![
                MirLocalDecl {
                    ty: MirType::ExternRef("JsObject".to_string()),
                    source_info: MirSourceInfo {
                        span: MirSpan {
                            filename: "test.rs".to_string(),
                            line: 1,
                            column: 10,
                        },
                    },
                },
                MirLocalDecl {
                    ty: MirType::ExternRef("JsObject".to_string()),
                    source_info: MirSourceInfo {
                        span: MirSpan {
                            filename: "test.rs".to_string(),
                            line: 2,
                            column: 10,
                        },
                    },
                },
            ],
            source_info: MirSourceInfo {
                span: MirSpan {
                    filename: "test.rs".to_string(),
                    line: 1,
                    column: 1,
                },
            },
        };
        
        // Lower the MIR function to WasmIR
        let result = context.lower_function(&mir_func);
        assert!(result.is_ok());
        
        let wasmir_func = result.unwrap();
        assert_eq!(wasmir_func.name, "use_externref");
        assert_eq!(wasmir_func.signature.params.len(), 1);
        assert_eq!(wasmir_func.signature.returns, None);
        
        // Check that ownership annotations were added
        assert!(!wasmir_func.ownership_annotations.is_empty());
        
        // Check that JS interop capability was detected
        assert!(wasmir_func.capabilities.contains(&Capability::JsInterop));
        
        // Validate the function
        assert!(wasmir_func.validate().is_ok());
    }
}
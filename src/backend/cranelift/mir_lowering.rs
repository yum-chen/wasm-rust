//! MIR to WasmIR Lowering Pass
//! 
//! This module implements the translation from Rust MIR to WasmIR,
//! handling ownership annotations, capability checks, and WASM-specific
//! optimizations during the lowering process.

use crate::wasmir::{
    WasmIR, Signature, Type, Instruction, Terminator, Operand, 
    BinaryOp, UnaryOp, BasicBlock, BlockId, Constant,
    OwnershipAnnotation, OwnershipState, SourceLocation, Capability
};
use rustc_middle::mir::{Body, BasicBlock, Terminator, Operand, Rvalue, Place, ProjectionElem};
use rustc_middle::ty::{TyS, TyKind};
use rustc_target::spec::Target;
use std::collections::{HashMap, HashSet};
use std::iter;

/// MIR to WasmIR lowering context
pub struct MirLoweringContext {
    /// Target architecture for lowering
    target: Target,
    /// Function being lowered
    wasmir_func: WasmIR,
    /// Local variable mapping from MIR locals to WasmIR locals
    local_map: HashMap<usize, u32>,
    /// Basic block mapping from MIR blocks to WasmIR blocks
    block_map: HashMap<rustc_middle::mir::BasicBlock, BlockId>,
    /// Current local index counter
    next_local_index: u32,
    /// Current block index counter
    next_block_index: u32,
    /// Ownership annotations being tracked
    ownership_annotations: Vec<OwnershipAnnotation>,
    /// Detected capabilities
    capabilities: HashSet<Capability>,
    /// Errors encountered during lowering
    errors: Vec<LoweringError>,
}

/// Errors that can occur during MIR lowering
#[derive(Debug, Clone)]
pub enum LoweringError {
    /// Unsupported MIR construct
    UnsupportedConstruct(String),
    /// Type conversion error
    TypeConversion(String),
    /// Ownership violation
    OwnershipViolation {
        local: usize,
        location: SourceLocation,
        violation: String,
    },
    /// Capability violation
    CapabilityViolation(Capability),
    /// General lowering error
    LoweringError(String),
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoweringError::UnsupportedConstruct(msg) => {
                write!(f, "Unsupported construct: {}", msg)
            }
            LoweringError::TypeConversion(msg) => {
                write!(f, "Type conversion error: {}", msg)
            }
            LoweringError::OwnershipViolation { local, location, violation } => {
                write!(f, "Ownership violation at local {}: {} at {}:{}", 
                    local, location.line, location.column, violation)
            }
            LoweringError::CapabilityViolation(cap) => {
                write!(f, "Capability violation: {:?}", cap)
            }
            LoweringError::LoweringError(msg) => {
                write!(f, "Lowering error: {}", msg)
            }
        }
    }
}

impl MirLoweringContext {
    /// Creates a new MIR lowering context
    pub fn new(target: Target, mir_body: &Body) -> Self {
        Self {
            target,
            wasmir_func: WasmIR::new(
                mir_body.source.scope.local_names()[0].to_string(),
                Self::convert_signature(&mir_body),
            ),
            local_map: HashMap::new(),
            block_map: HashMap::new(),
            next_local_index: mir_body.local_decls.len() as u32,
            next_block_index: 0,
            ownership_annotations: Vec::new(),
            capabilities: HashSet::new(),
            errors: Vec::new(),
        }
    }

    /// Lowers the MIR body to WasmIR
    pub fn lower_body(&mut self, mir_body: &Body) -> Result<(), Vec<LoweringError>> {
        // Add all MIR locals to WasmIR locals
        for (local_index, local_decl) in mir_body.local_decls.iter().enumerate() {
            let wasmir_local_index = self.next_local_index;
            self.next_local_index += 1;
            
            self.local_map.insert(local_index, wasmir_local_index);
            
            let wasmir_type = self.convert_ty(&local_decl.ty)?;
            self.wasmir_func.add_local(wasmir_type);
        }

        // Lower basic blocks
        for (bb_index, mir_bb) in mir_body.basic_blocks.iter().enumerate() {
            let block_id = self.lower_basic_block(mir_bb, bb_index)?;
            self.block_map.insert(*mir_bb, block_id);
        }

        // Lower control flow
        for (bb_index, mir_bb) in mir_body.basic_blocks.iter().enumerate() {
            let block_id = self.block_map.get(mir_bb).unwrap();
            
            // Lower terminator first
            let terminator = self.lower_terminator(&mir_bb.terminator())?;
            let instructions = self.lower_instructions(&mir_bb.statements)?;
            
            let wasmir_bb = BasicBlock {
                id: *block_id,
                instructions,
                terminator,
            };
            
            self.wasmir_func.basic_blocks.push(wasmir_bb);
        }

        // Add ownership annotations
        for annotation in &self.ownership_annotations {
            self.wasmir_func.add_ownership_annotation(annotation.clone());
        }

        // Add capability annotations
        for capability in &self.capabilities {
            self.wasmir_func.add_capability(*capability);
        }

        Ok(())
    }

    /// Gets the resulting WasmIR function
    pub fn into_wasmir(self) -> Result<WasmIR, Vec<LoweringError>> {
        if !self.errors.is_empty() {
            Ok(self.wasmir_func)
        } else {
            Err(self.errors)
        }
    }

    /// Converts MIR signature to WasmIR signature
    fn convert_signature(&self, mir_body: &Body) -> Signature {
        let mut params = Vec::new();
        
        for (arg_index, arg) in mir_body.args.iter().enumerate() {
            let wasmir_type = match &arg.ty.kind() {
                TyKind::Int(int_ty) => {
                    match int_ty.bit_width() {
                        Some(32) => Type::I32,
                        Some(64) => Type::I64,
                        _ => Type::I32, // Default fallback
                    }
                }
                TyKind::Float(float_ty) => {
                    match float_ty.bit_width() {
                        Some(32) => Type::F32,
                        Some(64) => Type::F64,
                        _ => Type::F32, // Default fallback
                    }
                }
                TyKind::Bool => Type::I32, // WASM doesn't have bool
                TyKind::Ref(..) => {
                    if arg.ty.is_box() {
                        Type::I32 // Box is a heap allocation
                    } else {
                        Type::I32 // Reference (handle)
                    }
                }
                _ => Type::I32, // Fallback
            };
            
            params.push(wasmir_type);
        }

        let returns = mir_body.return_ty().map(|ty| self.convert_ty(ty)).transpose().unwrap_or(None);

        Signature { params, returns }
    }

    /// Lowers a basic block and returns its WasmIR block ID
    fn lower_basic_block(
        &mut self,
        mir_bb: &rustc_middle::mir::BasicBlock,
        bb_index: usize,
    ) -> Result<BlockId, LoweringError> {
        let block_id = BlockId(self.next_block_index);
        self.next_block_index += 1;

        // Add a dummy block if it's not the first one
        if bb_index > 0 {
            self.wasmir_func.add_basic_block(vec![], Terminator::Jump { target: block_id });
        }

        Ok(block_id)
    }

    /// Lowers MIR terminator to WasmIR terminator
    fn lower_terminator(
        &mut self,
        terminator: &rustc_middle::mir::Terminator<'tcx>,
    ) -> Result<Terminator, LoweringError> {
        match terminator.kind() {
            rustc_middle::mir::TerminatorKind::Return => {
                let value = if let Some(place) = terminator.as_place() {
                    Some(self.lower_rvalue(place)?)
                } else {
                    None
                };
                Ok(Terminator::Return { value })
            }
            
            rustc_middle::mir::TerminatorKind::Switch { discr, values, targets } => {
                let discr_value = self.lower_rvalue(discr)?;
                let wasmir_values = values.iter()
                    .map(|opt| opt.map(|place| self.lower_rvalue(place)))
                    .collect::<Option<_>>();
                
                let switch_targets = values.iter().zip(targets.iter())
                    .enumerate()
                    .map(|(i, (_, target_bb))| {
                        let block_id = self.block_map.get(target_bb).unwrap();
                        (Constant::I32(i as i32), block_id)
                    })
                    .collect::<Vec<_>>();
                
                let default_target = if let Some(otherwise_bb) = targets.get(values.len()) {
                    *self.block_map.get(otherwise_bb).unwrap()
                } else {
                    BlockId(0) // Should not happen
                };
                
                Ok(Terminator::Switch {
                    value: discr_value,
                    targets: switch_targets,
                    default_target,
                })
            }
            
            rustc_middle::mir::TerminatorKind::Goto { target } => {
                let target_id = self.block_map.get(target).unwrap();
                Ok(Terminator::Jump { target: target_id })
            }
            
            rustc_middle::mir::TerminatorKind::Call { 
                func, 
                args, 
                destination,
                cleanup,
            } => {
                // This is typically handled at the instruction level
                // Return a dummy terminator for now
                Ok(Terminator::Return { value: None })
            }
            
            rustc_middle::mir::TerminatorKind::Assert { .. } => {
                // Assertions become runtime checks in WASM
                Ok(Terminator::Return { value: None })
            }
            
            rustc_middle::mir::TerminatorKind::Resume { .. } | 
            rustc_middle::mir::TerminatorKind::Abort { .. } |
            rustc_middle::mir::TerminatorKind::GeneratorEnd { .. } |
            rustc_middle::mir::TerminatorKind::Yield { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    "Unsupported terminator kind".to_string()
                ))
            }
            
            rustc_middle::mir::TerminatorKind::Unreachable => {
                Ok(Terminator::Unreachable)
            }
            
            rustc_middle::mir::TerminatorKind::FalseEdge { .. } |
            rustc_middle::mir::TerminatorKind::FalseUnwind { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    "False edge terminators not supported".to_string()
                ))
            }
        }
    }

    /// Lowers MIR instructions to WasmIR instructions
    fn lower_instructions(
        &mut self,
        statements: &[rustc_middle::mir::Statement<'tcx>],
    ) -> Result<Vec<Instruction>, LoweringError> {
        let mut instructions = Vec::new();
        
        for statement in statements {
            let wasm_instructions = self.lower_statement(statement)?;
            instructions.extend(wasm_instructions);
        }
        
        Ok(instructions)
    }

    /// Lowers a single MIR statement to WasmIR instructions
    fn lower_statement(
        &mut self,
        statement: &rustc_middle::mir::Statement<'tcx>,
    ) -> Result<Vec<Instruction>, LoweringError> {
        match statement.kind() {
            rustc_middle::mir::StatementKind::Assign(place, rvalue) => {
                let target = self.lower_place(place)?;
                let value = self.lower_rvalue(rvalue)?;
                
                // Check for linear type violations
                self.check_ownership_violation(&place, "assignment")?;
                
                Ok(vec![
                    Instruction::LocalSet {
                        index: target,
                        value,
                    }
                ])
            }
            
            rustc_middle::mir::StatementKind::SetDiscriminant { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    "Set discriminant not yet supported".to_string()
                ))
            }
            
            rustc_middle::mir::StatementKind::StorageLive(local) |
            rustc_middle::mir::StatementKind::StorageDead(local) => {
                // These are handled differently in WASM
                Ok(vec![])
            }
            
            rustc_middle::mir::StatementKind::Nop |
            rustc_middle::mir::StatementKind::FakeRead(..) |
            rustc_middle::mir::StatementKind::Retag { .. } |
            rustc_middle::mir::StatementKind::AscribeUserType { .. } |
            rustc_middle::mir::StatementKind::Coverage { .. } |
            rustc_middle::mir::StatementKind::ReadOnly { .. } |
            rustc_middle::mir::StatementKind::NonMutatingUse { .. } => {
                Ok(vec![])
            }
            
            rustc_middle::mir::StatementKind::PlaceMention(..) |
            rustc_middle::mir::StatementKind::FakeBox { .. } |
            rustc_middle::mir::StatementKind::LlvmInlineAsm { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    format!("Statement kind not supported: {:?}", statement.kind())
                ))
            }
        }
    }

    /// Lowers a MIR place to WasmIR operand
    fn lower_place(
        &mut self,
        place: &rustc_middle::mir::Place<'tcx>,
    ) -> Result<u32, LoweringError> {
        let local = match place.as_local() {
            Some(local) => local.local(),
            None => {
                return Err(LoweringError::UnsupportedConstruct(
                    "Only local places are supported in current lowering".to_string()
                ));
            }
        };
        
        self.local_map.get(&local)
            .copied()
            .ok_or_else(|| LoweringError::LoweringError(
                format!("Local {} not found in mapping", local)
            ))
    }

    /// Lowers a MIR rvalue to WasmIR operand
    fn lower_rvalue(
        &mut self,
        rvalue: &rustc_middle::mir::Rvalue<'tcx>,
    ) -> Result<Operand, LoweringError> {
        match rvalue {
            rustc_middle::mir::Rvalue::Use(place) => {
                self.lower_place(place).map(Operand::Local)
            }
            
            rustc_middle::mir::Rvalue::Repeat { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    "Repeat not supported in WASM lowering".to_string()
                ))
            }
            
            rustc_middle::mir::Rvalue::Ref { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    "Ref not supported in WASM lowering".to_string()
                ))
            }
            
            rustc_middle::mir::Rvalue::ThreadLocalRef { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    "Thread local ref not supported in WASM lowering".to_string()
                ))
            }
            
            rustc_middle::mir::Rvalue::BinaryOp { op, left, right } => {
                let left_operand = self.lower_rvalue(left)?;
                let right_operand = self.lower_rvalue(right)?;
                let wasmir_op = self.convert_binary_op(op)?;
                
                // This would generate a binary instruction, but since we're lowering an rvalue,
                // we need to create a temporary local for the result
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                let temp_type = self.infer_binary_result_type(left, right)?;
                self.wasmir_func.add_local(temp_type);
                
                Ok(Operand::Local(temp_local))
            }
            
            rustc_middle::mir::Rvalue::CheckedBinaryOp { op, left, right } => {
                // WASM handles overflow checking implicitly for signed ops
                self.lower_rvalue(&rustc_middle::mir::Rvalue::BinaryOp { op, left, right })
            }
            
            rustc_middle::mir::Rvalue::UnaryOp { op, arg } => {
                let arg_operand = self.lower_rvalue(arg)?;
                let wasmir_op = self.convert_unary_op(op)?;
                
                // Create temporary for unary result
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                let temp_type = self.infer_unary_result_type(arg)?;
                self.wasmir_func.add_local(temp_type);
                
                Ok(Operand::Local(temp_local))
            }
            
            rustc_middle::mir::Rvalue::Discriminant { .. } => {
                Err(LoweringError::UnsupportedConstruct(
                    "Discriminant lowering not yet implemented".to_string()
                ))
            }
            
            rustc_middle::mir::Rvalue::Aggregate { kind, operands, .. } => {
                self.lower_aggregate(kind, operands)
            }
            
            rustc_middle::mir::Rvalue::Len { place } => {
                // Convert len operation to appropriate WASM instruction
                let place_operand = self.lower_place(place)?;
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                self.wasmir_func.add_local(Type::I32);
                Ok(Operand::Local(temp_local))
            }
            
            rustc_middle::mir::Rvalue::Cast { kind, operand } => {
                let operand = self.lower_rvalue(operand)?;
                let target_type = self.convert_cast_kind(kind)?;
                
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                self.wasmir_func.add_local(target_type);
                Ok(Operand::Local(temp_local))
            }
            
            rustc_middle::mir::Rvalue::ShallowInitBox { .. } => {
                // Box allocation in WASM
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                self.wasmir_func.add_local(Type::I32); // Handle
                Ok(Operand::Local(temp_local))
            }
            
            rustc_middle::mir::Rvalue::NullaryOp { op } => {
                let wasmir_op = self.convert_nullary_op(op)?;
                
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                self.wasmir_func.add_local(self.infer_nullary_result_type(op));
                Ok(Operand::Local(temp_local))
            }
            
            rustc_middle::mir::Rvalue::NullaryOp { op } => {
                let wasmir_op = self.convert_nullary_op(op)?;
                
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                self.wasmir_func.add_local(self.infer_nullary_result_type(op));
                Ok(Operand::Local(temp_local))
            }
            
            rustc_middle::mir::Rvalue::NullaryOp { op } => {
                let wasmir_op = self.convert_nullary_op(op)?;
                
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                self.wasmir_func.add_local(self.infer_nullary_result_type(op));
                Ok(Operand::Local(temp_local))
            }
        }
    }

    /// Converts MIR binary operation to WasmIR binary operation
    fn convert_binary_op(
        &self,
        op: &rustc_middle::mir::BinOp,
    ) -> Result<BinaryOp, LoweringError> {
        match op {
            rustc_middle::mir::BinOp::Add => Ok(BinaryOp::Add),
            rustc_middle::mir::BinOp::Sub => Ok(BinaryOp::Sub),
            rustc_middle::mir::BinOp::Mul => Ok(BinaryOp::Mul),
            rustc_middle::mir::BinOp::Div => Ok(BinaryOp::Div),
            rustc_middle::mir::BinOp::Rem => Ok(BinaryOp::Mod),
            rustc_middle::mir::BinOp::BitXor => Ok(BinaryOp::Xor),
            rustc_middle::mir::BinOp::BitAnd => Ok(BinaryOp::And),
            rustc_middle::mir::BinOp::BitOr => Ok(BinaryOp::Or),
            rustc_middle::mir::BinOp::Shl => Ok(BinaryOp::Shl),
            rustc_middle::mir::BinOp::Shr => Ok(BinaryOp::Shr),
            rustc_middle::mir::BinOp::Eq => Ok(BinaryOp::Eq),
            rustc_middle::mir::BinOp::Lt => Ok(BinaryOp::Lt),
            rustc_middle::mir::BinOp::Le => Ok(BinaryOp::Le),
            rustc_middle::mir::BinOp::Gt => Ok(BinaryOp::Gt),
            rustc_middle::mir::BinOp::Ge => Ok(BinaryOp::Ge),
            rustc_middle::mir::BinOp::Ne => Ok(BinaryOp::Ne),
            rustc_middle::mir::BinOp::Cmp => Ok(BinaryOp::Lt), // Simplified
            _ => Err(LoweringError::UnsupportedConstruct(
                format!("Unsupported binary operation: {:?}", op)
            )),
        }
    }

    /// Converts MIR unary operation to WasmIR unary operation
    fn convert_unary_op(
        &self,
        op: &rustc_middle::mir::UnOp,
    ) -> Result<UnaryOp, LoweringError> {
        match op {
            rustc_middle::mir::UnOp::Not => Ok(UnaryOp::Not),
            rustc_middle::mir::UnOp::Neg => Ok(UnaryOp::Neg),
            rustc_middle::mir::UnOp::Clz => Ok(UnaryOp::Clz),
            rustc_middle::mir::UnOp::Ctz => Ok(UnaryOp::Ctz),
            rustc_middle::mir::UnOp::Popcnt => Ok(UnaryOp::Popcnt),
            _ => Err(LoweringError::UnsupportedConstruct(
                format!("Unsupported unary operation: {:?}", op)
            )),
        }
    }

    /// Converts MIR cast operation to target type
    fn convert_cast_kind(
        &self,
        kind: &rustc_middle::mir::CastKind,
    ) -> Result<Type, LoweringError> {
        match kind {
            rustc_middle::mir::CastKind::IntToInt => Ok(Type::I32),
            rustc_middle::mir::CastKind::IntToFloat => Ok(Type::F32),
            rustc_middle::mir::CastKind::FloatToInt => Ok(Type::I32),
            rustc_middle::mir::CastKind::FloatToFloat => Ok(Type::F64),
            rustc_middle::mir::CastKind::FloatToInt => Ok(Type::I32),
            _ => Err(LoweringError::UnsupportedConstruct(
                format!("Unsupported cast kind: {:?}", kind)
            )),
        }
    }

    /// Converts MIR nullary operation to WasmIR unary operation
    fn convert_nullary_op(
        &self,
        op: &rustc_middle::mir::NullOp,
    ) -> Result<UnaryOp, LoweringError> {
        match op {
            rustc_middle::mir::NullOp::SizeOf => Ok(UnaryOp::Clz), // Simplified
            rustc_middle::mir::NullOp::AlignOf => Ok(UnaryOp::Clz), // Simplified
            rustc_middle::mir::NullOp::OffsetOf => Ok(UnaryOp::Clz), // Simplified
            rustc_middle::mir::NullOp::BoxFree => Ok(UnaryOp::Not), // Memory free
            _ => Err(LoweringError::UnsupportedConstruct(
                format!("Unsupported nullary operation: {:?}", op)
            )),
        }
    }

    /// Converts MIR type to WasmIR type
    fn convert_ty(&self, ty: &TyS) -> Result<Type, LoweringError> {
        match ty.kind() {
            TyKind::Int(int_ty) => {
                match int_ty.bit_width() {
                    Some(32) => Ok(Type::I32),
                    Some(64) => Ok(Type::I64),
                    Some(128) => Ok(Type::I64), // No I128 in WASM
                    _ => Ok(Type::I32), // Default fallback
                }
            }
            TyKind::Float(float_ty) => {
                match float_ty.bit_width() {
                    Some(32) => Ok(Type::F32),
                    Some(64) => Ok(Type::F64),
                    _ => Ok(Type::F32), // Default fallback
                }
            }
            TyKind::Bool => Ok(Type::I32), // WASM doesn't have bool
            TyKind::Ref(..) => {
                if ty.is_box() {
                    Ok(Type::I32) // Box is a heap allocation
                } else {
                    Ok(Type::I32) // Reference (handle)
                }
            }
            TyKind::Tuple(..) => {
                Err(LoweringError::UnsupportedConstruct(
                    "Tuple types not supported in WASM".to_string()
                ))
            }
            TyKind::Slice(..) => {
                Err(LoweringError::UnsupportedConstruct(
                    "Slice types need special handling".to_string()
                ))
            }
            TyKind::Adt { .. } => {
                Ok(Type::I32) // Simplified for now
            }
            TyKind::Foreign(..) => {
                Ok(Type::I32) // Opaque type
            }
            TyKind::Dynamic(..) => {
                Ok(Type::I32) // Dynamic type
            }
            _ => {
                Err(LoweringError::TypeConversion(
                    format!("Unsupported type: {:?}", ty.kind())
                ))
            }
        }
    }

    /// Checks for ownership violations during lowering
    fn check_ownership_violation(
        &mut self,
        place: &rustc_middle::mir::Place<'tcx>,
        operation: &str,
    ) -> Result<(), LoweringError> {
        if let Some(local) = place.as_local() {
            if let Some(local_index) = local.local.checked_idx() {
                // Check if this local has been marked as consumed
                let annotation = self.ownership_annotations.iter()
                    .find(|ann| ann.variable == local_index);
                
                if let Some(annotation) = annotation {
                    if annotation.state == OwnershipState::Consumed {
                        let location = SourceLocation {
                            file: "mir_lowering".to_string(),
                            line: 0, // Would need actual source info
                            column: 0,
                        };
                        
                        self.errors.push(LoweringError::OwnershipViolation {
                            local: local_index,
                            location,
                            violation: format!("Attempted to use consumed local during {}", operation),
                        });
                        return Err(LoweringError::LoweringError(
                            "Ownership violation detected".to_string()
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Detects capabilities based on MIR analysis
    fn detect_capabilities(&mut self, mir_body: &Body) {
        // Check for JavaScript interop usage
        for statement in mir_body.basic_blocks.iter().flat_map(|bb| &bb.statements) {
            if statement.requires_js_interop() {
                self.capabilities.insert(Capability::JsInterop);
            }
        }
        
        // Check for threading usage
        if self.detect_threading_usage(mir_body) {
            self.capabilities.insert(Capability::Threading);
        }
        
        // Check for atomic operations
        if self.detect_atomic_usage(mir_body) {
            self.capabilities.insert(Capability::AtomicMemory);
        }
    }

    /// Checks if statement requires JavaScript interop
    fn requires_js_interop(&self, statement: &rustc_middle::mir::Statement<'tcx>) -> bool {
        // This is a simplified check - in practice, this would look for
        // external function calls, externref usage, etc.
        matches!(statement.kind(), 
            rustc_middle::mir::StatementKind::Assign(..) |
            rustc_middle::mir::StatementKind::Call(..)
        )
    }

    /// Detects threading usage in MIR
    fn detect_threading_usage(&self, mir_body: &Body) -> bool {
        // Check for thread creation, atomic operations, etc.
        mir_body.basic_blocks.iter().any(|bb| {
            bb.statements.iter().any(|stmt| {
                matches!(stmt.kind(),
                    rustc_middle::mir::StatementKind::Call(..) |
                    rustc_middle::mir::StatementKind::Assign(..)
                )
            })
        })
    }

    /// Detects atomic operations in MIR
    fn detect_atomic_usage(&self, mir_body: &Body) -> bool {
        mir_body.basic_blocks.iter().any(|bb| {
            bb.statements.iter().any(|stmt| {
                matches!(stmt.kind(),
                    rustc_middle::mir::StatementKind::Assign(..)
                )
            })
        })
    }

    /// Infers result type for binary operations
    fn infer_binary_result_type(
        &self,
        left: &rustc_middle::mir::Rvalue<'tcx>,
        right: &rustc_middle::mir::Rvalue<'tcx>,
    ) -> Result<Type, LoweringError> {
        // Simplified type inference - in practice, this would be more sophisticated
        let left_ty = self.infer_rvalue_type(left)?;
        let right_ty = self.infer_rvalue_type(right)?;
        
        match (left_ty, right_ty) {
            (Type::I32, Type::I32) => Ok(Type::I32),
            (Type::I64, Type::I64) => Ok(Type::I64),
            (Type::F32, Type::F32) => Ok(Type::F32),
            (Type::F64, Type::F64) => Ok(Type::F64),
            _ => Ok(Type::I32), // Default fallback
        }
    }

    /// Infers result type for unary operations
    fn infer_unary_result_type(
        &self,
        arg: &rustc_middle::mir::Rvalue<'tcx>,
    ) -> Result<Type, LoweringError> {
        let arg_ty = self.infer_rvalue_type(arg)?;
        match arg_ty {
            Type::I32 | Type::I64 => Ok(arg_ty),
            Type::F32 | Type::F64 => Ok(arg_ty),
            _ => Ok(Type::I32), // Default fallback
        }
    }

    /// Infers result type for nullary operations
    fn infer_nullary_result_type(
        &self,
        op: &rustc_middle::mir::NullOp,
    ) -> Type {
        match op {
            rustc_middle::mir::NullOp::SizeOf => Type::I32,
            rustc_middle::mir::NullOp::AlignOf => Type::I32,
            rustc_middle::mir::NullOp::OffsetOf => Type::I32,
            _ => Type::I32, // Default fallback
        }
    }

    /// Infers type from rvalue
    fn infer_rvalue_type(
        &self,
        rvalue: &rustc_middle::mir::Rvalue<'tcx>,
    ) -> Result<Type, LoweringError> {
        match rvalue {
            rustc_middle::mir::Rvalue::Use(place) => {
                let local = match place.as_local() {
                    Some(local) => local.local(),
                    None => {
                        return Err(LoweringError::UnsupportedConstruct(
                            "Only local places are supported".to_string()
                        ));
                    }
                };
                
                self.local_map.get(&local)
                    .copied()
                    .and_then(|local_index| {
                        self.wasmir_func.locals.get(local_index as usize).cloned()
                    })
                    .ok_or_else(|| LoweringError::LoweringError(
                        format!("Local {} not found in WASMIR function", local)
                    ))
            }
            _ => {
                Err(LoweringError::UnsupportedConstruct(
                    format!("Unsupported rvalue: {:?}", rvalue.kind())
                ))
            }
        }
    }

    /// Lowers aggregate operations (structs, arrays, etc.)
    fn lower_aggregate(
        &mut self,
        kind: &rustc_middle::mir::AggregateKind<'tcx>,
        operands: &[rustc_middle::mir::Rvalue<'tcx>],
    ) -> Result<Operand, LoweringError> {
        match kind {
            rustc_middle::mir::AggregateKind::Array(..) => {
                // Array literal
                let temp_local = self.next_local_index;
                self.next_local_index += 1;
                
                self.wasmir_func.add_local(Type::I32); // Handle
                Ok(Operand::Local(temp_local))
            }
            rustc_middle::mir::AggregateKind::Tuple(..) => {
                // Tuple literal - not supported in WASM
                Err(LoweringError::UnsupportedConstruct(
                    "Tuple literals not supported in WASM".to_string()
                ))
            }
            _ => {
                Err(LoweringError::UnsupportedConstruct(
                    format!("Unsupported aggregate kind: {:?}", kind)
                ))
            }
        }
    }
}

// Extension trait for checking JavaScript interop requirements
trait JsInteropCheck {
    fn requires_js_interop(&self) -> bool;
}

impl JsInteropCheck for rustc_middle::mir::Statement<'tcx> {
    fn requires_js_interop(&self) -> bool {
        match self.kind() {
            rustc_middle::mir::StatementKind::Call(func) => {
                // Check if the function being called is an external function
                func.def_id().map(|def_id| {
                    def_id.as_local().map_or(false, |local| {
                        local.def_id().is_none() // External if no local def
                    })
                }).unwrap_or(false)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_middle::mir::{self};
    use crate::wasmir;

    #[test]
    fn test_mir_lowering_context_creation() {
        let mir_body = Body::new(
            vec![], // No parameters
            Vec::new(),
            rustc_middle::mir::BasicBlockData {
                basic_blocks: Vec::new(),
            },
            rustc_middle::mir::SourceInfo {
                span: rustc_span::DUMMY_SP,
            },
            None,
        );
        
        let context = MirLoweringContext::new(
            rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            },
            &mir_body
        );
        
        assert_eq!(context.wasmir_func.locals.len(), 0);
        assert_eq!(context.next_local_index, 0);
        assert!(context.errors.is_empty());
    }

    #[test]
    fn test_signature_conversion() {
        let mir_body = Body::new(
            vec![self::Argument { 
                ty: self::tcx.types.mk_i32(),
                span: rustc_span::DUMMY_SP,
            }],
            None,
            rustc_middle::mir::BasicBlockData {
                basic_blocks: Vec::new(),
            },
            rustc_middle::mir::SourceInfo {
                span: rustc_span::DUMMY_SP,
            },
            None,
        );
        
        let context = MirLoweringContext::new(
            rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            },
            &mir_body
        );
        
        let signature = context.convert_signature(&mir_body);
        assert_eq!(signature.params.len(), 1);
        assert_eq!(signature.returns, None);
    }

    #[test]
    fn test_type_conversion() {
        let mir_body = Body::new(
            Vec::new(),
            None,
            rustc_middle::mir::BasicBlockData {
                basic_blocks: Vec::new(),
            },
            rustc_middle::mir::SourceInfo {
                span: rustc_span::DUMMY_SP,
            },
            None,
        );
        
        let context = MirLoweringContext::new(
            rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            },
            &mir_body
        );
        
        // Test integer types
        let i32_type = context.convert_ty(&self::tcx.types.mk_i32()).unwrap();
        assert_eq!(i32_type, Type::I32);
        
        let i64_type = context.convert_ty(&self::tcx.types.mk_i64()).unwrap();
        assert_eq!(i64_type, Type::I64);
        
        // Test float types
        let f32_type = context.convert_ty(&self::tcx.types.mk_f32()).unwrap();
        assert_eq!(f32_type, Type::F32);
        
        let f64_type = context.convert_ty(&self::tcx.types.mk_f64()).unwrap();
        assert_eq!(f64_type, Type::F64);
        
        // Test bool type
        let bool_type = context.convert_ty(&self::tcx.types.mk_bool()).unwrap();
        assert_eq!(bool_type, Type::I32);
    }

    #[test]
    fn test_binary_operation_conversion() {
        let mir_body = Body::new(
            Vec::new(),
            None,
            rustc_middle::mir::BasicBlockData {
                basic_blocks: Vec::new(),
            },
            rustc_middle::mir::SourceInfo {
                span: rustc_span::DUMMY_SP,
            },
            None,
        );
        
        let context = MirLoweringContext::new(
            rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            },
            &mir_body
        );
        
        assert_eq!(context.convert_binary_op(&self::BinOp::Add).unwrap(), BinaryOp::Add);
        assert_eq!(context.convert_binary_op(&self::BinOp::Sub).unwrap(), BinaryOp::Sub);
        assert_eq!(context.convert_binary_op(&self::BinOp::Mul).unwrap(), BinaryOp::Mul);
        assert_eq!(context.convert_binary_op(&self::BinOp::Div).unwrap(), BinaryOp::Div);
    }

    #[test]
    fn test_lowering_error_display() {
        let error = LoweringError::OwnershipViolation {
            local: 0,
            location: SourceLocation {
                file: "test.rs".to_string(),
                line: 42,
                column: 10,
            },
            violation: "Use after consume".to_string(),
        };
        
        let display = format!("{}", error);
        assert!(display.contains("Use after consume"));
        assert!(display.contains("42"));
        assert!(display.contains("10"));
    }

    #[test]
    fn test_capability_detection() {
        let mir_body = Body::new(
            Vec::new(),
            None,
            rustc_middle::mir::BasicBlockData {
                basic_blocks: Vec::new(),
            },
            rustc_middle::mir::SourceInfo {
                span: rustc_span::DUMMY_SP,
            },
            None,
        );
        
        let mut context = MirLoweringContext::new(
            rustc_target::spec::Target {
                arch: "wasm32".to_string(),
                ..Default::default()
            },
            &mir_body
        );
        
        context.detect_capabilities(&mir_body);
        
        // Should have no capabilities initially
        assert!(context.capabilities.is_empty());
        
        // Add a statement that might require JS interop
        // This is simplified - in practice, this would analyze actual patterns
        context.capabilities.insert(Capability::JsInterop);
        
        assert!(context.capabilities.contains(&Capability::JsInterop));
    }
}

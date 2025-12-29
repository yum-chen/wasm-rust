//! MIR to WasmIR lowering for WasmRust
//! 
//! This module implements the transformation from Rust MIR to WasmIR,
//! preserving Rust semantics while enabling WASM-specific optimizations.

use rustc_middle::mir::*;
use rustc_middle::ty::{self as Ty, TyKind, AdtDef, Instance};
use rustc_target::spec::Target;
use wasm::wasmir::{WasmIR, Instruction, Terminator, BasicBlock, BlockId, Type, Signature, Operand, BinaryOp, UnaryOp, Constant, Capability, OwnershipAnnotation, OwnershipState, SourceLocation};
use wasm::host::get_host_capabilities;
use std::collections::HashMap;

/// MIR lowering context for handling complex transformations
pub struct MirLoweringContext {
    /// Target architecture information
    target: Target,
    /// MIR body being lowered
    mir_body: Body<'static>,
    /// Mapping from MIR locals to WasmIR locals
    local_mapping: HashMap<Local, u32>,
    /// Mapping from MIR basic blocks to WasmIR blocks
    block_mapping: HashMap<BasicBlock, BlockId>,
    /// Current function being built
    current_function: Option<WasmIR>,
    /// Ownership tracking for linear types
    ownership_tracker: OwnershipTracker,
    /// Debug information preservation
    debug_info: DebugInfoPreserver,
    /// Capability annotations
    capabilities: Vec<Capability>,
}

/// Tracks ownership state for linear types during lowering
struct OwnershipTracker {
    /// Current ownership state for each local
    states: HashMap<Local, OwnershipState>,
    /// Linear type annotations
    linear_types: HashMap<Local, Type>,
}

/// Preserves debug information during lowering
struct DebugInfoPreserver {
    /// Source locations for instructions
    source_locations: HashMap<Instruction, SourceLocation>,
    /// Variable name mapping
    variable_names: HashMap<Local, String>,
}

impl MirLoweringContext {
    /// Creates a new MIR lowering context
    pub fn new(target: Target, mir_body: &Body<'_>) -> Self {
        Self {
            target,
            mir_body: unsafe { std::mem::transmute(mir_body) }, // Safe for static lifetime in lowering
            local_mapping: HashMap::new(),
            block_mapping: HashMap::new(),
            current_function: None,
            ownership_tracker: OwnershipTracker {
                states: HashMap::new(),
                linear_types: HashMap::new(),
            },
            debug_info: DebugInfoPreserver {
                source_locations: HashMap::new(),
                variable_names: HashMap::new(),
            },
            capabilities: Vec::new(),
        }
    }

    /// Lowers the entire MIR body to WasmIR
    pub fn lower_body(&mut self, mir_body: &Body<'_>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        // Create function signature
        let signature = self.lower_signature(mir_body);
        let mut wasmir_func = WasmIR::new(
            mir_body.source.fn_name.to_string(),
            signature,
        );
        
        // Add capabilities based on function analysis
        self.add_function_capabilities(&mut wasmir_func, mir_body);
        
        // Lower local variables
        if let Err(err) = self.lower_locals(&mut wasmir_func, mir_body) {
            errors.extend(err);
        }
        
        // Lower basic blocks
        if let Err(err) = self.lower_basic_blocks(&mut wasmir_func, mir_body) {
            errors.extend(err);
        }
        
        // Add ownership annotations
        self.add_ownership_annotations(&mut wasmir_func);
        
        // Validate the resulting WasmIR
        if let Err(validation_err) = wasmir_func.validate() {
            errors.push(format!("Validation error: {}", validation_err));
        }
        
        self.current_function = Some(wasmir_func);
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Converts the lowered function to WasmIR
    pub fn into_wasmir(self) -> Result<WasmIR, String> {
        self.current_function.ok_or_else(|| "No function lowered".to_string())
    }

    /// Lowers function signature from MIR
    fn lower_signature(&self, mir_body: &Body<'_>) -> Signature {
        let mut params = Vec::new();
        
        // Lower parameters
        for local in mir_body.args_iter() {
            let local_decl = &mir_body.local_decls[local];
            let wasm_type = self.lower_type(local_decl.ty);
            params.push(wasm_type);
        }
        
        // Lower return type
        let returns = match mir_body.return_ty().kind() {
            TyKind::Tuple(tys) if tys.is_empty() => None,
            _ => Some(self.lower_type(mir_body.return_ty())),
        };
        
        Signature { params, returns }
    }

    /// Lowers local variable declarations
    fn lower_locals(&mut self, wasmir_func: &mut WasmIR, mir_body: &Body<'_>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        for (local, local_decl) in mir_body.local_decls.iter_enumerated() {
            let wasm_type = self.lower_type(local_decl.ty);
            let local_index = wasmir_func.add_local(wasm_type);
            
            self.local_mapping.insert(local, local_index);
            
            // Track variable name for debug info
            if let Some(name) = mir_body.var_debug_info.iter().find(|info| info.place.local == local) {
                self.debug_info.variable_names.insert(local, name.name.to_string());
            }
            
            // Check if this is a linear type
            if self.is_linear_type(local_decl.ty) {
                self.ownership_tracker.linear_types.insert(local, wasm_type);
                self.ownership_tracker.states.insert(local, OwnershipState::Owned);
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Lowers all basic blocks from MIR
    fn lower_basic_blocks(&mut self, wasmir_func: &mut WasmIR, mir_body: &Body<'_>) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        
        for (bb_data, mir_bb) in mir_body.basic_blocks.iter_enumerated() {
            let block_id = BlockId(bb_data.index());
            self.block_mapping.insert(bb_data, block_id);
            
            // Lower instructions in the basic block
            let mut instructions = Vec::new();
            
            for statement in &mir_bb.statements {
                match self.lower_statement(statement) {
                    Ok(Some(instr)) => instructions.push(instr),
                    Ok(None) => {} // No instruction generated
                    Err(err) => errors.push(err),
                }
            }
            
            // Lower terminator
            match self.lower_terminator(&mir_bb.terminator(), mir_bb) {
                Ok(terminator) => {
                    let wasm_bb = BasicBlock {
                        id: block_id,
                        instructions,
                        terminator,
                    };
                    wasmir_func.basic_blocks.push(wasm_bb);
                }
                Err(err) => errors.push(err),
            }
        }
        
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Lowers a MIR statement to WasmIR instruction
    fn lower_statement(&mut self, statement: &Statement<'_>) -> Result<Option<Instruction>, String> {
        match statement.kind {
            StatementKind::Assign(box (place, rvalue)) => {
                let place_operand = self.lower_place(&place)?;
                let value_operand = self.lower_rvalue(&rvalue)?;
                
                Ok(Some(Instruction::LocalSet {
                    index: self.get_local_index(&place.local)?,
                    value: value_operand,
                }))
            }
            StatementKind::StorageLive(local) => {
                // Mark local as live for ownership tracking
                if let Some(state) = self.ownership_tracker.states.get_mut(&local) {
                    *state = OwnershipState::Owned;
                }
                Ok(Some(Instruction::Nop))
            }
            StatementKind::StorageDead(local) => {
                // Mark local as dead for ownership tracking
                if let Some(state) = self.ownership_tracker.states.get_mut(&local) {
                    *state = OwnershipState::Consumed;
                }
                Ok(Some(Instruction::Nop))
            }
            StatementKind::Nop => Ok(Some(Instruction::Nop)),
            _ => Ok(None), // Other statements not implemented yet
        }
    }

    /// Lowers a MIR terminator to WasmIR terminator
    fn lower_terminator(&mut self, terminator: &Terminator<'_>, _mir_bb: &BasicBlockData<'_>) -> Result<Terminator, String> {
        match terminator.kind {
            TerminatorKind::Return => {
                Ok(Terminator::Return { value: None })
            }
            TerminatorKind::Goto { target } => {
                let target_block = self.block_mapping[&target];
                Ok(Terminator::Jump { target: target_block })
            }
            TerminatorKind::SwitchInt { discr, targets, .. } => {
                let discr_operand = self.lower_operand(&discr)?;
                let mut switch_targets = Vec::new();
                
                for (value, target) in targets.iter() {
                    let target_block = self.block_mapping[&target];
                    switch_targets.push((Operand::Constant(Constant::I32(value as i32)), target_block));
                }
                
                let default_target = self.block_mapping[&targets.otherwise()];
                
                Ok(Terminator::Switch {
                    value: discr_operand,
                    targets: switch_targets,
                    default_target,
                })
            }
            TerminatorKind::Call { func, args, destination, .. } => {
                // Lower function call
                let func_name = match func {
                    Operand::Constant(const_val) => {
                        match const_val.kind {
                            ConstKind::Function(handle, _) => {
                                self.mir_body.source.fn_name.to_string() // Simplified
                            }
                            _ => return Err("Invalid function operand".to_string()),
                        }
                    }
                    _ => return Err("Function call not implemented for non-constant functions".to_string()),
                };
                
                let mut wasm_args = Vec::new();
                for arg in args {
                    wasm_args.push(self.lower_operand(arg)?);
                }
                
                // Create call instruction
                let call_instr = Instruction::Call {
                    func_ref: 0, // Placeholder - would be resolved during linking
                    args: wasm_args,
                };
                
                // Store result in destination
                let dest_index = self.get_local_index(&destination.local)?;
                let set_instr = Instruction::LocalSet {
                    index: dest_index,
                    value: Operand::StackValue(0), // Placeholder for call result
                };
                
                // For now, return a simple terminator
                Ok(Terminator::Return { value: None })
            }
            TerminatorKind::Unreachable => Ok(Terminator::Unreachable),
            TerminatorKind::Resume => Ok(Terminator::Panic { 
                message: Some(Operand::Constant(Constant::String("Resume".to_string()))) 
            }),
            TerminatorKind::Abort => Ok(Terminator::Panic { 
                message: Some(Operand::Constant(Constant::String("Abort".to_string()))) 
            }),
            _ => Err(format!("Unsupported terminator: {:?}", terminator.kind)),
        }
    }

    /// Lowers a MIR place to WasmIR operand
    fn lower_place(&mut self, place: &Place<'_>) -> Result<Operand, String> {
        if place.projection.is_empty() {
            // Simple local variable
            Ok(Operand::Local(self.get_local_index(&place.local)?))
        } else {
            // Complex place with projections - simplified for now
            Ok(Operand::Local(self.get_local_index(&place.local)?))
        }
    }

    /// Lowers a MIR rvalue to WasmIR operand
    fn lower_rvalue(&mut self, rvalue: &Rvalue<'_>) -> Result<Operand, String> {
        match rvalue.kind {
            RvalueKind::Use(operand) => self.lower_operand(operand),
            RvalueKind::BinaryOp(bin_op, left, right) => {
                let left_operand = self.lower_operand(left)?;
                let right_operand = self.lower_operand(right)?;
                let wasm_bin_op = self.lower_binary_op(bin_op);
                
                // Create binary operation instruction
                Ok(Operand::StackValue(0)) // Placeholder - would need SSA handling
            }
            RvalueKind::UnaryOp(un_op, operand) => {
                let operand = self.lower_operand(operand)?;
                let wasm_un_op = self.lower_unary_op(un_op);
                
                Ok(Operand::StackValue(0)) // Placeholder
            }
            RvalueKind::Constant(const_val) => self.lower_constant(const_val),
            RvalueKind::Ref(_, _, place) => {
                let place_operand = self.lower_place(place)?;
                // Handle reference creation
                Ok(place_operand)
            }
            _ => Err(format!("Unsupported rvalue: {:?}", rvalue.kind)),
        }
    }

    /// Lowers a MIR operand to WasmIR operand
    fn lower_operand(&mut self, operand: &Operand<'_>) -> Result<Operand, String> {
        match operand {
            Operand::Copy(place) => self.lower_place(place),
            Operand::Move(place) => {
                // Handle move semantics for ownership tracking
                if let Some(local) = place.as_local() {
                    if let Some(state) = self.ownership_tracker.states.get_mut(&local) {
                        *state = OwnershipState::Moved;
                    }
                }
                self.lower_place(place)
            }
            Operand::Constant(const_val) => self.lower_constant(const_val),
        }
    }

    /// Lowers a constant value to WasmIR constant
    fn lower_constant(&self, const_val: &Constant<'_>) -> Result<Operand, String> {
        match const_val.kind {
            ConstKind::Value(ty, const_val) => {
                match const_val {
                    ConstValue::Scalar(scalar) => {
                        match scalar {
                            Scalar::Int(int) => {
                                let int_val = int.assert_bits(self.target.pointer_width());
                                if int_val.size() <= 32 {
                                    Ok(Operand::Constant(Constant::I32(int_val.to_i32())))
                                } else {
                                    Ok(Operand::Constant(Constant::I64(int_val.to_i64())))
                                }
                            }
                            Scalar::Float(float) => {
                                if float.is_nan() {
                                    Ok(Operand::Constant(Constant::F64(f64::NAN)))
                                } else {
                                    Ok(Operand::Constant(Constant::F64(float.to_f64())))
                                }
                            }
                        }
                    }
                    _ => Err("Unsupported constant value".to_string()),
                }
            }
            ConstKind::Zst => Ok(Operand::Constant(Constant::I32(0))),
            _ => Err(format!("Unsupported constant kind: {:?}", const_val.kind)),
        }
    }

    /// Lowers a Rust type to WasmIR type
    fn lower_type(&self, ty: Ty<'_>) -> Type {
        match ty.kind() {
            TyKind::Int(int_ty) => {
                match int_ty.kind() {
                    IntTyKind::I8 | IntTyKind::U8 => Type::I32,
                    IntTyKind::I16 | IntTyKind::U16 => Type::I32,
                    IntTyKind::I32 | IntTyKind::U32 => Type::I32,
                    IntTyKind::I64 | IntTyKind::U64 => Type::I64,
                    IntTyKind::I128 | IntTyKind::U128 => Type::I64, // Map to 64-bit
                }
            }
            TyKind::Uint(uint_ty) => {
                match uint_ty.kind() {
                    UintTyKind::U8 | UintTyKind::U16 | UintTyKind::U32 => Type::I32,
                    UintTyKind::U64 => Type::I64,
                    UintTyKind::U128 => Type::I64,
                    UintTyKind::Usize => Type::I32, // WASM32 target
                }
            }
            TyKind::Float(float_ty) => {
                match float_ty.kind() {
                    FloatTyKind::F32 => Type::F32,
                    FloatTyKind::F64 => Type::F64,
                }
            }
            TyKind::Bool => Type::I32,
            TyKind::Ref(_, inner_ty, _) | TyKind::RawPtr(inner_ty, _) => {
                match inner_ty.kind() {
                    TyKind::Adt(adt_def, _) if self.is_externref_type(adt_def) => {
                        Type::ExternRef("externref".to_string())
                    }
                    _ => Type::I32, // Map references to i32 handles
                }
            }
            TyKind::Tuple(tys) => {
                if tys.is_empty() {
                    Type::Void
                } else if tys.len() == 1 {
                    self.lower_type(tys[0])
                } else {
                    Type::Struct {
                        fields: tys.iter().map(|ty| self.lower_type(ty)).collect(),
                    }
                }
            }
            TyKind::Array(elem_ty, len) => {
                Type::Array {
                    element_type: Box::new(self.lower_type(elem_ty)),
                    size: Some(len.try_to_target_usize(&self.target).unwrap_or(0) as u32),
                }
            }
            TyKind::Slice(elem_ty) => {
                Type::Array {
                    element_type: Box::new(self.lower_type(elem_ty)),
                    size: None,
                }
            }
            TyKind::Adt(adt_def, _) => {
                if self.is_externref_type(adt_def) {
                    Type::ExternRef(adt_def.name().to_string())
                } else {
                    Type::Struct {
                        fields: adt_def.variants.iter()
                            .flat_map(|variant| variant.fields.iter())
                            .map(|field| self.lower_type(field.ty(self.mir_body)))
                            .collect(),
                    }
                }
            }
            _ => Type::I32, // Fallback
        }
    }

    /// Lowers binary operation from MIR to WasmIR
    fn lower_binary_op(&self, bin_op: BinOp) -> BinaryOp {
        match bin_op {
            BinOp::Add => BinaryOp::Add,
            BinOp::Sub => BinaryOp::Sub,
            BinOp::Mul => BinaryOp::Mul,
            BinOp::Div => BinaryOp::Div,
            BinOp::Rem => BinaryOp::Mod,
            BinOp::BitAnd => BinaryOp::And,
            BinOp::BitOr => BinaryOp::Or,
            BinOp::BitXor => BinaryOp::Xor,
            BinOp::Shl => BinaryOp::Shl,
            BinOp::Shr => BinaryOp::Shr,
            BinOp::Eq => BinaryOp::Eq,
            BinOp::Ne => BinaryOp::Ne,
            BinOp::Lt => BinaryOp::Lt,
            BinOp::Le => BinaryOp::Le,
            BinOp::Gt => BinaryOp::Gt,
            BinOp::Ge => BinaryOp::Ge,
            _ => BinaryOp::Add, // Fallback
        }
    }

    /// Lowers unary operation from MIR to WasmIR
    fn lower_unary_op(&self, un_op: UnOp) -> UnaryOp {
        match un_op {
            UnOp::Neg => UnaryOp::Neg,
            UnOp::Not => UnaryOp::Not,
            UnOp::PtrMetadata => UnaryOp::Neg, // Fallback
        }
    }

    /// Gets the WasmIR local index for a MIR local
    fn get_local_index(&self, local: &Local) -> Result<u32, String> {
        self.local_mapping.get(local)
            .copied()
            .ok_or_else(|| format!("Local not mapped: {:?}", local))
    }

    /// Checks if a type is a linear type
    fn is_linear_type(&self, ty: Ty<'_>) -> bool {
        match ty.kind() {
            TyKind::Adt(adt_def, _) => {
                // Check if this is a WasmRust linear type
                adt_def.name().contains("Linear") || 
                adt_def.name().contains("Owned") ||
                adt_def.name().contains("Resource")
            }
            TyKind::Ref(_, inner_ty, _) => self.is_linear_type(*inner_ty),
            _ => false,
        }
    }

    /// Checks if an ADT represents an ExternRef type
    fn is_externref_type(&self, adt_def: &AdtDef) -> bool {
        adt_def.name().contains("ExternRef") || 
        adt_def.name().contains("JsValue") ||
        adt_def.name().contains("Object")
    }

    /// Adds capability annotations to function
    fn add_function_capabilities(&mut self, wasmir_func: &mut WasmIR, mir_body: &Body<'_>) {
        let caps = get_host_capabilities();
        
        // Add threading capability if async function
        if mir_body.return_ty().is_async() {
            wasmir_func.add_capability(Capability::Threading);
        }
        
        // Add JS interop capability if ExternRef types are present
        for local_decl in mir_body.local_decls.iter() {
            if self.type_contains_externref(local_decl.ty) {
                wasmir_func.add_capability(Capability::JsInterop);
                break;
            }
        }
        
        // Add atomic memory capability if atomic operations are used
        for bb in mir_body.basic_blocks.iter() {
            for statement in &bb.statements {
                if self.statement_uses_atomics(statement) {
                    wasmir_func.add_capability(Capability::AtomicMemory);
                    break;
                }
            }
        }
    }

    /// Checks if a type contains ExternRef
    fn type_contains_externref(&self, ty: Ty<'_>) -> bool {
        match ty.kind() {
            TyKind::Adt(adt_def, _) => self.is_externref_type(adt_def),
            TyKind::Ref(_, inner_ty, _) => self.type_contains_externref(*inner_ty),
            TyKind::Tuple(tys) => tys.iter().any(|ty| self.type_contains_externref(ty)),
            TyKind::Array(elem_ty, _) => self.type_contains_externref(elem_ty),
            _ => false,
        }
    }

    /// Checks if a statement uses atomic operations
    fn statement_uses_atomics(&self, statement: &Statement<'_>) -> bool {
        match statement.kind {
            StatementKind::Assign(box (_, rvalue)) => {
                match rvalue.kind {
                    RvalueKind::AtomicOp(_, _, _) |
                    RvalueKind::AtomicLoad(_) |
                    RvalueKind::AtomicStore(_, _) => true,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// Adds ownership annotations to the function
    fn add_ownership_annotations(&mut self, wasmir_func: &mut WasmIR) {
        for (local, state) in &self.ownership_tracker.states {
            if let Some(ty) = self.ownership_tracker.linear_types.get(local) {
                let annotation = OwnershipAnnotation {
                    variable: self.local_mapping.get(local).copied().unwrap_or(0),
                    state: *state,
                    source_location: SourceLocation {
                        file: self.mir_body.source.span.file.name.to_string(),
                        line: self.mir_body.source.span.lo().line,
                        column: 0,
                    },
                };
                wasmir_func.add_ownership_annotation(annotation);
            }
        }
    }
}

// Legacy MIRLowerer for backward compatibility
pub struct MIRLowerer {
    target: Target,
    locals: Vec<Local>,
    blocks: Vec<BasicBlock>,
    next_block_id: usize,
}

impl MIRLowerer {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            locals: Vec::new(),
            blocks: Vec::new(),
            next_block_id: 0,
        }
    }

    pub fn lower_mir(&mut self, mir_body: &Body<'_>) -> WasmIR {
        // Create WasmIR function signature
        let signature = self.lower_signature(mir_body);
        let mut wasm_func = WasmIR::new("lowered_function".to_string(), signature);
        
        // Lower local declarations
        for (local, local_decl) in mir_body.local_decls.iter_enumerated() {
            self.lower_local_decl(&mut wasm_func, local, local_decl);
        }
        
        // Lower basic blocks
        for (bb_index, mir_bb) in mir_body.basic_blocks.iter_enumerated() {
            let wasm_bb = self.lower_basic_block(mir_bb, bb_index);
            wasm_func.add_basic_block(wasm_bb);
        }
        
        wasm_func
    }

    fn lower_signature(&self, mir_body: &Body<'_>) -> Signature {
        // Simplified signature lowering
        let mut params = Vec::new();
        for arg in mir_body.args_iter() {
            params.push(self.lower_type(arg.ty()));
        }
        
        let returns = match mir_body.return_ty() {
            ty => match ty.kind() {
                TyKind::Tuple(tys) if tys.is_empty() => None,
                _ => Some(self.lower_type(mir_body.return_ty())),
            },
        };
        
        Signature { params, returns }
    }

    fn lower_local_decl(&mut self, wasm_func: &mut WasmIR, local: Local, local_decl: &LocalDecl<'_>) {
        let wasm_type = self.lower_type(local_decl.ty);
        let local_index = wasm_func.add_local(wasm_type);
        
        // Track mapping
        while self.locals.len() <= local.local_index() {
            self.locals.push(Local::INVALID);
        }
        self.locals[local.local_index()] = local;
    }

    fn lower_basic_block(&mut self, mir_bb: &BasicBlock<'_>, bb_index: usize) -> BasicBlock {
        let block_id = BlockId::new(self.next_block_id);
        self.next_block_id += 1;
        
        let mut instructions = Vec::new();
        
        // Lower statements
        for statement in &mir_bb.statements {
            match statement {
                Statement::Assign(place, rvalue) => {
                    let value = self.lower_rvalue(rvalue);
                    let operand = self.lower_place(place);
                    instructions.push(Instruction::LocalSet {
                        index: operand.local_index(),
                        value,
                    });
                }
                Statement::StorageLive(local) => {
                    // Handle liveness
                    instructions.push(Instruction::Nop);
                }
                Statement::StorageDead(local) => {
                    // Handle liveness
                    instructions.push(Instruction::Nop);
                }
                _ => {
                    // Other statements as needed
                    instructions.push(Instruction::Nop);
                }
            }
        }
        
        // Lower terminator
        let terminator = self.lower_terminator(&mir_bb.terminator());
        
        BasicBlock::new(block_id, instructions, terminator)
    }

    fn lower_rvalue(&mut self, rvalue: &Rvalue<'_>) -> Operand {
        match rvalue.kind() {
            RvalueKind::Use(operand) => self.lower_operand(operand),
            RvalueKind::BinaryOp(bin_op, left, right) => {
                let left_op = self.lower_operand(left);
                let right_op = self.lower_operand(right);
                let wasm_bin_op = self.lower_binary_op(*bin_op);
                
                // Create a temporary local for the result
                let result_type = self.lower_type(rvalue.ty());
                let result_index = self.locals.len();
                self.locals.push(Local::from_usize(result_index));
                
                Operand::Local(result_index)
            }
            RvalueKind::UnaryOp(un_op, operand) => {
                let op = self.lower_operand(operand);
                let wasm_un_op = self.lower_unary_op(*un_op);
                
                // Create a temporary local for the result
                let result_type = self.lower_type(rvalue.ty());
                let result_index = self.locals.len();
                self.locals.push(Local::from_usize(result_index));
                
                Operand::Local(result_index)
            }
            RvalueKind::Constant(const_val) => {
                match const_val.kind() {
                    ConstKind::Value(ty, const_val) => {
                        match const_val {
                            ConstValue::Scalar(scalar) => {
                                match scalar {
                                    Scalar::Int(int) => {
                                        let int_val = int.assert_bits(self.target.pointer_width());
                                        if int_val.size() == 32 {
                                            Operand::I32(int_val.to_i32() as i32)
                                        } else if int_val.size() == 64 {
                                            Operand::I64(int_val.to_i64() as i64)
                                        } else {
                                            Operand::I32(0) // Fallback
                                        }
                                    }
                                    }
                                    Scalar::Float(float) => {
                                        if float.is_nan() {
                                            Operand::F64(f64::NAN)
                                        } else {
                                            Operand::F64(float.to_f64())
                                        }
                                    }
                                    }
                                }
                            }
                            _ => Operand::I32(0), // Fallback
                        }
                    }
                    _ => Operand::I32(0), // Fallback
                }
            }
            _ => Operand::I32(0), // Fallback
        }
    }

    fn lower_place(&mut self, place: &Place<'_>) -> Operand {
        match place.ty().kind() {
            TyKind::Ref(ty) => {
                self.lower_operand(place.projection.last().unwrap_or(&PlaceElem::Local(local!())))
            }
            _ => {
                self.lower_operand(place.projection.last().unwrap_or(&PlaceElem::Local(local!())))
            }
        }
    }

    fn lower_operand(&mut self, operand: &Operand<'_>) -> Operand {
        match operand {
            Operand::Copy(place) => self.lower_place(place),
            Operand::Move(place) => self.lower_place(place),
            Operand::Local(local) => Operand::Local(local.local_index()),
            _ => Operand::I32(0), // Fallback
        }
    }

    fn lower_terminator(&mut self, terminator: &Terminator<'_>) -> Terminator {
        match terminator.kind() {
            TerminatorKind::Return { .. } => {
                Terminator::Return {
                    value: None, // Simplified
                }
            }
            TerminatorKind::Goto { target } => {
                Terminator::Goto {
                    target: BlockId::new(target.index()),
                }
            }
            TerminatorKind::Switch { .. } => {
                Terminator::Goto {
                    target: BlockId::new(0), // Simplified
                }
            }
            TerminatorKind::Unreachable => {
                Terminator::Unreachable
            }
            TerminatorKind::Call { .. } => {
                Terminator::Goto {
                    target: BlockId::new(0), // Simplified
                }
            }
            _ => Terminator::Unreachable,
        }
    }

    fn lower_type(&self, ty: Ty<'_>) -> Type {
        match ty.kind() {
            TyKind::Int(int_ty) => {
                match int_ty.kind() {
                    IntTyKind::I8 | IntTyKind::U8 => Type::I32,
                    IntTyKind::I16 | IntTyKind::U16 => Type::I32,
                    IntTyKind::I32 | IntTyKind::U32 => Type::I32,
                    IntTyKind::I64 | IntTyKind::U64 => Type::I64,
                    IntTyKind::I128 | IntTyKind::U128 => Type::I64, // Map 128-bit to 64-bit
                }
            }
            TyKind::Float(float_ty) => {
                match float_ty.kind() {
                    FloatTyKind::F32 => Type::F32,
                    FloatTyKind::F64 => Type::F64,
                }
            }
            TyKind::Ref(ty) => Type::Ref(format!("{:?}", ty.kind())),
            TyKind::Tuple(tys) => {
                if tys.is_empty() {
                    Type::Void
                } else if tys.len() == 1 {
                    self.lower_type(tys[0])
                } else {
                    Type::Ref(format!("tuple_{}", tys.len()))
                }
            }
            TyKind::Bool => Type::I32, // Map bool to i32
            _ => Type::Ref(format!("{:?}", ty.kind())),
        }
    }

    fn lower_binary_op(&self, bin_op: BinOp) -> BinaryOp {
        match bin_op {
            BinOp::Add => BinaryOp::Add,
            BinOp::Sub => BinaryOp::Sub,
            BinOp::Mul => BinaryOp::Mul,
            BinOp::Div => BinaryOp::Div,
            BinOp::Rem => BinaryOp::Mod,
            BinOp::BitAnd => BinaryOp::And,
            BinOp::BitOr => BinaryOp::Or,
            BinOp::BitXor => BinaryOp::Xor,
            BinOp::Shl => BinaryOp::Shl,
            BinOp::Shr => BinaryOp::Shr,
            BinOp::Eq => BinaryOp::Eq,
            BinOp::Lt => BinaryOp::Lt,
            BinOp::Le => BinaryOp::Le,
            _ => BinaryOp::Add, // Fallback
        }
    }

    fn lower_unary_op(&self, un_op: UnOp) -> UnaryOp {
        match un_op {
            UnOp::Neg => UnaryOp::Neg,
            UnOp::Not => UnaryOp::Not,
            _ => UnaryOp::Neg, // Fallback
        }
    }
}

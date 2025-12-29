//! WasmIR to WebAssembly code generation
//! 
//! This module implements transformation from WasmIR to WebAssembly bytecode,
//! including WASM-specific optimizations and streaming layout optimization.

use wasm::wasmir::{WasmIR, Instruction, Terminator, BasicBlock, Type, Signature, Operand, BinaryOp, UnaryOp, Constant, Capability};
use crate::backend::{CompilationResult, BackendError};
use std::collections::HashMap;

/// WebAssembly code generator
pub struct WasmCodegen {
    /// Generated WASM bytes
    wasm_bytes: Vec<u8>,
    /// Function section content
    function_section: Vec<u8>,
    /// Code section content
    code_section: Vec<u8>,
    /// Type section content
    type_section: Vec<u8>,
    /// Export section content
    export_section: Vec<u8>,
    /// Import section content
    import_section: Vec<u8>,
    /// Function index mapping
    function_index_map: HashMap<String, u32>,
    /// Type index mapping
    type_index_map: HashMap<Signature, u32>,
    /// Streaming layout optimizer
    streaming_optimizer: StreamingLayoutOptimizer,
    /// WASM-specific optimizations
    wasm_optimizer: WasmOptimizer,
}

/// Streaming layout optimizer for fast WASM instantiation
pub struct StreamingLayoutOptimizer {
    /// Function layout order
    function_order: Vec<String>,
    /// Code segment boundaries
    segment_boundaries: Vec<usize>,
    /// Optimization enabled
    enabled: bool,
}

/// WASM-specific optimizer
pub struct WasmOptimizer {
    /// Optimization passes to apply
    optimization_passes: Vec<Box<dyn OptimizationPass>>,
    /// Optimization enabled
    enabled: bool,
}

/// Trait for optimization passes
trait OptimizationPass {
    /// Applies optimization to WasmIR
    fn apply(&self, wasmir: &mut WasmIR) -> Result<(), String>;
    /// Gets optimization pass name
    fn name(&self) -> &'static str;
}

/// Dead code elimination optimization
struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn apply(&self, wasmir: &mut WasmIR) -> Result<(), String> {
        let mut used_locals = wasmir.used_locals();
        
        // Remove unused local variables
        wasmir.locals.retain(|index| used_locals.contains(index));
        
        // Remove unreachable basic blocks
        self.remove_unreachable_blocks(wasmir);
        
        Ok(())
    }
    
    fn name(&self) -> &'static str {
        "dead_code_elimination"
    }
}

impl DeadCodeElimination {
    fn remove_unreachable_blocks(&self, wasmir: &mut WasmIR) {
        let mut reachable_blocks = std::collections::HashSet::new();
        let mut worklist = vec![0]; // Start with entry block
        
        while let Some(block_index) = worklist.pop() {
            if !reachable_blocks.contains(&block_index) {
                reachable_blocks.insert(block_index);
                
                if let Some(block) = wasmir.basic_blocks.get(block_index) {
                    // Add successor blocks to worklist
                    match &block.terminator {
                        Terminator::Branch { then_block, else_block, .. } => {
                            worklist.push(then_block.0);
                            worklist.push(else_block.0);
                        }
                        Terminator::Jump { target } => {
                            worklist.push(target.0);
                        }
                        Terminator::Switch { default_target, targets, .. } => {
                            worklist.push(default_target.0);
                            for (_, target) in targets {
                                worklist.push(target.0);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        // Keep only reachable blocks
        wasmir.basic_blocks.retain(|block| reachable_blocks.contains(&block.id.0));
    }
}

/// Constant folding optimization
struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn apply(&self, wasmir: &mut WasmIR) -> Result<(), String> {
        for block in &mut wasmir.basic_blocks {
            for instruction in &mut block.instructions {
                self.fold_constants(instruction);
            }
        }
        Ok(())
    }
    
    fn name(&self) -> &'static str {
        "constant_folding"
    }
}

impl ConstantFolding {
    fn fold_constants(&self, instruction: &mut Instruction) {
        match instruction {
            Instruction::BinaryOp { op, left, right } => {
                if let (Operand::Constant(left_const), Operand::Constant(right_const)) = (left, right) {
                    if let Some(result) = self.evaluate_binary_op(*op, left_const, right_const) {
                        *instruction = Instruction::LocalSet {
                            index: 0, // Placeholder
                            value: Operand::Constant(result),
                        };
                    }
                }
            }
            _ => {}
        }
    }
    
    fn evaluate_binary_op(&self, op: BinaryOp, left: &Constant, right: &Constant) -> Option<Constant> {
        match (left, right) {
            (Constant::I32(l), Constant::I32(r)) => {
                let result = match op {
                    BinaryOp::Add => l + r,
                    BinaryOp::Sub => l - r,
                    BinaryOp::Mul => l * r,
                    BinaryOp::Div => l.checked_div(r)?,
                    BinaryOp::Mod => l.checked_rem(r)?,
                    BinaryOp::And => l & r,
                    BinaryOp::Or => l | r,
                    BinaryOp::Xor => l ^ r,
                    BinaryOp::Shl => l.checked_shl(r.min(31) as u32)?,
                    BinaryOp::Shr => l.checked_shr(r.min(31) as u32)?,
                    BinaryOp::Eq => (l == r) as i32,
                    BinaryOp::Ne => (l != r) as i32,
                    BinaryOp::Lt => (l < r) as i32,
                    BinaryOp::Le => (l <= r) as i32,
                    BinaryOp::Gt => (l > r) as i32,
                    BinaryOp::Ge => (l >= r) as i32,
                };
                Some(Constant::I32(result))
            }
            (Constant::I64(l), Constant::I64(r)) => {
                let result = match op {
                    BinaryOp::Add => l + r,
                    BinaryOp::Sub => l - r,
                    BinaryOp::Mul => l * r,
                    BinaryOp::Div => l.checked_div(r)?,
                    BinaryOp::Mod => l.checked_rem(r)?,
                    BinaryOp::And => l & r,
                    BinaryOp::Or => l | r,
                    BinaryOp::Xor => l ^ r,
                    BinaryOp::Shl => l.checked_shl(r.min(63) as u32)?,
                    BinaryOp::Shr => l.checked_shr(r.min(63) as u32)?,
                    BinaryOp::Eq => (l == r) as i64,
                    BinaryOp::Ne => (l != r) as i64,
                    BinaryOp::Lt => (l < r) as i64,
                    BinaryOp::Le => (l <= r) as i64,
                    BinaryOp::Gt => (l > r) as i64,
                    BinaryOp::Ge => (l >= r) as i64,
                };
                Some(Constant::I64(result))
            }
            _ => None,
        }
    }
}

/// Instruction selection optimization
struct InstructionSelection;

impl OptimizationPass for InstructionSelection {
    fn apply(&self, wasmir: &mut WasmIR) -> Result<(), String> {
        for block in &mut wasmir.basic_blocks {
            for instruction in &mut block.instructions {
                self.optimize_instruction(instruction);
            }
        }
        Ok(())
    }
    
    fn name(&self) -> &'static str {
        "instruction_selection"
    }
}

impl InstructionSelection {
    fn optimize_instruction(&self, instruction: &mut Instruction) {
        match instruction {
            Instruction::BinaryOp { op, left, right } => {
                // Optimize multiplication by power of 2 to shifts
                if *op == BinaryOp::Mul {
                    if let Operand::Constant(Constant::I32(const_val)) = right {
                        if const_val.is_power_of_two() && const_val > 0 {
                            let shift_amount = const_val.trailing_zeros();
                            *op = BinaryOp::Shl;
                            *right = Operand::Constant(Constant::I32(shift_amount as i32));
                        }
                    }
                }
                
                // Optimize division by power of 2 to shifts
                if *op == BinaryOp::Div {
                    if let Operand::Constant(Constant::I32(const_val)) = right {
                        if const_val.is_power_of_two() && const_val > 0 {
                            let shift_amount = const_val.trailing_zeros();
                            *op = BinaryOp::Shr;
                            *right = Operand::Constant(Constant::I32(shift_amount as i32));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl WasmCodegen {
    /// Creates a new WebAssembly code generator
    pub fn new() -> Self {
        Self {
            wasm_bytes: Vec::new(),
            function_section: Vec::new(),
            code_section: Vec::new(),
            type_section: Vec::new(),
            export_section: Vec::new(),
            import_section: Vec::new(),
            function_index_map: HashMap::new(),
            type_index_map: HashMap::new(),
            streaming_optimizer: StreamingLayoutOptimizer::new(),
            wasm_optimizer: WasmOptimizer::new(),
        }
    }

    /// Compiles WasmIR to WebAssembly bytecode
    pub fn compile(&mut self, wasmir: &mut WasmIR) -> Result<CompilationResult, BackendError> {
        // Apply WASM-specific optimizations
        self.wasm_optimizer.apply_optimizations(wasmir)?;
        
        // Apply streaming layout optimization
        self.streaming_optimizer.optimize_layout(wasmir)?;
        
        // Generate WASM sections
        self.generate_type_section(wasmir)?;
        self.generate_import_section(wasmir)?;
        self.generate_function_section(wasmir)?;
        self.generate_code_section(wasmir)?;
        self.generate_export_section(wasmir)?;
        
        // Assemble final WASM module
        self.assemble_wasm_module()?;
        
        Ok(CompilationResult {
            code: self.wasm_bytes.clone(),
            symbols: self.generate_symbol_table(wasmir),
            relocations: Vec::new(),
            metadata: crate::backend::CompilationMetadata {
                target: "wasm32-unknown-unknown".to_string(),
                optimization_level: crate::backend::OptimizationLevel::Standard,
                build_profile: crate::backend::BuildProfile::Development,
                timestamp: std::time::SystemTime::now(),
            },
        })
    }

    /// Generates the type section
    fn generate_type_section(&mut self, wasmir: &WasmIR) -> Result<(), BackendError> {
        self.type_section.clear();
        
        // Add function type
        let type_index = self.get_or_create_type_index(&wasmir.signature);
        
        // Build type section
        let mut section_bytes = Vec::new();
        section_bytes.push(0x01); // One type
        
        // Function type
        section_bytes.push(0x60); // Function type
        
        // Parameters
        section_bytes.push(wasmir.signature.params.len() as u8);
        for param_type in &wasmir.signature.params {
            section_bytes.push(self.type_to_wasm_type(param_type)?);
        }
        
        // Returns
        match &wasmir.signature.returns {
            Some(return_type) => {
                section_bytes.push(0x01); // One return
                section_bytes.push(self.type_to_wasm_type(return_type)?);
            }
            None => {
                section_bytes.push(0x00); // No returns
            }
        }
        
        self.type_section = section_bytes;
        Ok(())
    }

    /// Generates the import section
    fn generate_import_section(&mut self, wasmir: &WasmIR) -> Result<(), BackendError> {
        self.import_section.clear();
        
        // For now, assume no imports
        self.import_section.push(0x00); // No imports
        
        Ok(())
    }

    /// Generates the function section
    fn generate_function_section(&mut self, wasmir: &WasmIR) -> Result<(), BackendError> {
        self.function_section.clear();
        
        // Function section header
        self.function_section.push(0x01); // One function
        self.function_section.push(0x00); // Type index 0
        
        Ok(())
    }

    /// Generates the code section
    fn generate_code_section(&mut self, wasmir: &WasmIR) -> Result<(), BackendError> {
        self.code_section.clear();
        
        let mut function_body = Vec::new();
        
        // Function body
        let mut local_entries = Vec::new();
        
        // Add local variable declarations
        for (i, local_type) in wasmir.locals.iter().enumerate() {
            if i >= wasmir.signature.params.len() {
                local_entries.push(self.type_to_wasm_type(local_type)?);
            }
        }
        
        function_body.push(local_entries.len() as u8);
        for local_type in local_entries {
            function_body.push(0x01); // One local
            function_body.push(local_type);
        }
        
        // Generate instructions for each basic block
        for block in &wasmir.basic_blocks {
            for instruction in &block.instructions {
                self.encode_instruction(instruction, &mut function_body)?;
            }
            
            // Encode terminator
            self.encode_terminator(&block.terminator, &mut function_body)?;
        }
        
        // Add function body to code section
        let body_size = function_body.len();
        self.code_section.push(0x01); // One function
        self.code_section.extend_from_slice(&(body_size as u32).to_le_bytes());
        self.code_section.extend_from_slice(&function_body);
        
        Ok(())
    }

    /// Generates the export section
    fn generate_export_section(&mut self, wasmir: &WasmIR) -> Result<(), BackendError> {
        self.export_section.clear();
        
        // Export the main function
        let mut export_bytes = Vec::new();
        
        // Function name
        export_bytes.push(wasmir.name.len() as u8);
        export_bytes.extend_from_slice(wasmir.name.as_bytes());
        
        // Export kind (function)
        export_bytes.push(0x00);
        
        // Function index
        export_bytes.push(0x00);
        
        // Export section header
        self.export_section.push(0x01); // One export
        self.export_section.extend_from_slice(&export_bytes);
        
        Ok(())
    }

    /// Encodes a WasmIR instruction to WASM bytecode
    fn encode_instruction(&self, instruction: &Instruction, output: &mut Vec<u8>) -> Result<(), BackendError> {
        match instruction {
            Instruction::LocalGet { index } => {
                output.push(0x20); // local.get
                self.encode_u32(*index, output);
            }
            Instruction::LocalSet { index, value: _ } => {
                output.push(0x21); // local.set
                self.encode_u32(*index, output);
            }
            Instruction::BinaryOp { op, left: _, right: _ } => {
                // Emit operands would be handled by a proper register allocator
                // For now, just emit the operator
                match op {
                    BinaryOp::Add => output.push(0x6a), // i32.add
                    BinaryOp::Sub => output.push(0x6b), // i32.sub
                    BinaryOp::Mul => output.push(0x6c), // i32.mul
                    BinaryOp::Div => output.push(0x6d), // i32.div_s
                    BinaryOp::Mod => output.push(0x6f), // i32.rem_s
                    BinaryOp::And => output.push(0x71), // i32.and
                    BinaryOp::Or => output.push(0x72), // i32.or
                    BinaryOp::Xor => output.push(0x73), // i32.xor
                    BinaryOp::Shl => output.push(0x74), // i32.shl
                    BinaryOp::Shr => output.push(0x75), // i32.shr_s
                    BinaryOp::Eq => output.push(0x46), // i32.eq
                    BinaryOp::Ne => output.push(0x47), // i32.ne
                    BinaryOp::Lt => output.push(0x48), // i32.lt_s
                    BinaryOp::Le => output.push(0x49), // i32.le_s
                    BinaryOp::Gt => output.push(0x4a), // i32.gt_s
                    BinaryOp::Ge => output.push(0x4b), // i32.ge_s
                }
            }
            Instruction::UnaryOp { op, value: _ } => {
                match op {
                    UnaryOp::Neg => output.push(0x6a), // Use add with -1
                    UnaryOp::Not => output.push(0x6a), // Use xor with -1
                }
            }
            Instruction::Return { value: None } => {
                output.push(0x0b); // return
            }
            Instruction::Nop => {
                output.push(0x01); // nop
            }
            _ => {
                return Err(BackendError::Unsupported(
                    format!("Instruction not yet implemented: {:?}", instruction)
                ));
            }
        }
        Ok(())
    }

    /// Encodes a terminator to WASM bytecode
    fn encode_terminator(&self, terminator: &Terminator, output: &mut Vec<u8>) -> Result<(), BackendError> {
        match terminator {
            Terminator::Return { value: None } => {
                output.push(0x0b); // return
            }
            Terminator::Jump { target } => {
                output.push(0x0c); // br
                self.encode_u32(target.0, output);
            }
            Terminator::Unreachable => {
                output.push(0x00); // unreachable
            }
            _ => {
                return Err(BackendError::Unsupported(
                    format!("Terminator not yet implemented: {:?}", terminator)
                ));
            }
        }
        Ok(())
    }

    /// Converts a WasmIR type to WASM type
    fn type_to_wasm_type(&self, wasm_type: &Type) -> Result<u8, BackendError> {
        match wasm_type {
            Type::I32 => Ok(0x7f), // i32
            Type::I64 => Ok(0x7e), // i64
            Type::F32 => Ok(0x7d), // f32
            Type::F64 => Ok(0x7c), // f64
            _ => Err(BackendError::Unsupported(
                format!("Type not supported in WASM: {:?}", wasm_type)
            )),
        }
    }

    /// Gets or creates a type index for a signature
    fn get_or_create_type_index(&mut self, signature: &Signature) -> u32 {
        if let Some(&index) = self.type_index_map.get(signature) {
            index
        } else {
            let index = self.type_index_map.len() as u32;
            self.type_index_map.insert(signature.clone(), index);
            index
        }
    }

    /// Encodes a u32 value using LEB128 encoding
    fn encode_u32(&self, mut value: u32, output: &mut Vec<u8>) {
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            output.push(byte);
            if value == 0 {
                break;
            }
        }
    }

    /// Assembles the final WASM module
    fn assemble_wasm_module(&mut self) -> Result<(), BackendError> {
        self.wasm_bytes.clear();
        
        // WASM magic number and version
        self.wasm_bytes.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
        
        // Type section
        if !self.type_section.is_empty() {
            self.wasm_bytes.push(0x01); // Type section
            self.encode_section(&self.type_section);
        }
        
        // Import section
        if !self.import_section.is_empty() {
            self.wasm_bytes.push(0x02); // Import section
            self.encode_section(&self.import_section);
        }
        
        // Function section
        if !self.function_section.is_empty() {
            self.wasm_bytes.push(0x03); // Function section
            self.encode_section(&self.function_section);
        }
        
        // Export section
        if !self.export_section.is_empty() {
            self.wasm_bytes.push(0x07); // Export section
            self.encode_section(&self.export_section);
        }
        
        // Code section
        if !self.code_section.is_empty() {
            self.wasm_bytes.push(0x0a); // Code section
            self.encode_section(&self.code_section);
        }
        
        Ok(())
    }

    /// Encodes a section with length prefix
    fn encode_section(&mut self, section: &[u8]) {
        let length = section.len() as u32;
        self.encode_u32(length);
        self.wasm_bytes.extend_from_slice(section);
    }

    /// Generates symbol table for linking
    fn generate_symbol_table(&self, wasmir: &WasmIR) -> HashMap<String, u64> {
        let mut symbols = HashMap::new();
        
        // Add function symbol
        symbols.insert(wasmir.name.clone(), 0);
        
        symbols
    }
}

impl StreamingLayoutOptimizer {
    /// Creates a new streaming layout optimizer
    pub fn new() -> Self {
        Self {
            function_order: Vec::new(),
            segment_boundaries: Vec::new(),
            enabled: true,
        }
    }

    /// Optimizes function layout for streaming
    pub fn optimize_layout(&mut self, wasmir: &mut WasmIR) -> Result<(), BackendError> {
        if !self.enabled {
            return Ok(());
        }
        
        // Reorder basic blocks for optimal streaming
        self.reorder_basic_blocks(wasmir)?;
        
        // Create code segments for streaming
        self.create_code_segments(wasmir)?;
        
        Ok(())
    }

    /// Reorders basic blocks for optimal streaming
    fn reorder_basic_blocks(&mut self, wasmir: &mut WasmIR) -> Result<(), BackendError> {
        let mut visited = std::collections::HashSet::new();
        let mut new_order = Vec::new();
        let mut worklist = vec![0]; // Start with entry block
        
        while let Some(block_index) = worklist.pop() {
            if !visited.contains(&block_index) {
                visited.insert(block_index);
                
                if let Some(block) = wasmir.basic_blocks.get(block_index) {
                    new_order.push(block_index);
                    
                    // Add successor blocks to worklist
                    match &block.terminator {
                        Terminator::Branch { then_block, else_block, .. } => {
                            worklist.push(then_block.0);
                            worklist.push(else_block.0);
                        }
                        Terminator::Jump { target } => {
                            worklist.push(target.0);
                        }
                        Terminator::Switch { default_target, targets, .. } => {
                            worklist.push(default_target.0);
                            for (_, target) in targets {
                                worklist.push(target.0);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        
        // Reorder blocks
        let mut new_blocks = Vec::new();
        for block_index in new_order {
            if let Some(block) = wasmir.basic_blocks.iter().find(|b| b.id.0 == block_index) {
                new_blocks.push(block.clone());
            }
        }
        wasmir.basic_blocks = new_blocks;
        
        Ok(())
    }

    /// Creates code segments for streaming
    fn create_code_segments(&mut self, wasmir: &WasmIR) -> Result<(), BackendError> {
        let mut current_offset = 0;
        
        for block in &wasmir.basic_blocks {
            let block_size = block.instructions.len() + 1; // +1 for terminator
            
            if current_offset + block_size > 1024 { // 1KB segments
                self.segment_boundaries.push(current_offset);
                current_offset = 0;
            }
            
            current_offset += block_size;
        }
        
        if !self.segment_boundaries.is_empty() {
            self.segment_boundaries.push(current_offset);
        }
        
        Ok(())
    }
}

impl WasmOptimizer {
    /// Creates a new WASM optimizer
    pub fn new() -> Self {
        let mut optimization_passes: Vec<Box<dyn OptimizationPass>> = Vec::new();
        
        // Add optimization passes in order
        optimization_passes.push(Box::new(DeadCodeElimination));
        optimization_passes.push(Box::new(ConstantFolding));
        optimization_passes.push(Box::new(InstructionSelection));
        
        Self {
            optimization_passes,
            enabled: true,
        }
    }

    /// Applies all optimization passes to WasmIR
    pub fn apply_optimizations(&mut self, wasmir: &mut WasmIR) -> Result<(), BackendError> {
        if !self.enabled {
            return Ok(());
        }
        
        for pass in &self.optimization_passes {
            pass.apply(wasmir).map_err(|err| {
                BackendError::OptimizationFailed(format!("{}: {}", pass.name(), err))
            })?;
        }
        
        Ok(())
    }
    
    /// Enables or disables optimizations
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// Gets the list of optimization passes
    pub fn get_optimization_passes(&self) -> Vec<&'static str> {
        self.optimization_passes.iter().map(|pass| pass.name()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm::wasmir::*;

    #[test]
    fn test_wasm_codegen_creation() {
        let codegen = WasmCodegen::new();
        assert!(codegen.wasm_bytes.is_empty());
        assert!(codegen.function_section.is_empty());
        assert!(codegen.code_section.is_empty());
    }

    #[test]
    fn test_simple_function_compilation() {
        let mut codegen = WasmCodegen::new();
        
        let mut wasmir = WasmIR::new(
            "test_function".to_string(),
            Signature {
                params: vec![Type::I32, Type::I32],
                returns: Some(Type::I32),
            },
        );
        
        // Add a simple add function
        let block = BasicBlock {
            id: BlockId(0),
            instructions: vec![
                Instruction::LocalGet { index: 0 },
                Instruction::LocalGet { index: 1 },
                Instruction::BinaryOp {
                    op: BinaryOp::Add,
                    left: Operand::Local(0),
                    right: Operand::Local(1),
                },
            ],
            terminator: Terminator::Return { value: None },
        };
        
        wasmir.basic_blocks.push(block);
        
        let result = codegen.compile(&mut wasmir);
        assert!(result.is_ok());
        
        let compilation_result = result.unwrap();
        assert!(!compilation_result.code.is_empty());
        
        // Check for WASM magic number
        assert_eq!(&compilation_result.code[0..4], &[0x00, 0x61, 0x73, 0x6d]);
    }
}

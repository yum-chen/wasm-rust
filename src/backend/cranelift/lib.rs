//! WasmRust Cranelift Backend
//! 
//! This module provides a Cranelift-based codegen backend for WasmRust,
//! optimized for fast development compilation. It integrates with rustc's
//! codegen interface while adding WasmRust-specific optimizations.

use cranelift::codegen::*;
use cranelift::frontend::{FunctionBuilder, Variable};
use cranelift::ir::{Function, InstBuilder, Signature, AbiParam, AbiParam};
use cranelift::isa::TargetIsa;
use cranelift::settings::FlagsOrIsa;
use cranelift::prelude::*;
use cranelift::metaprogram::Syntax;
use cranelift::entity::EntityRef;
use rustc_middle::mir::Body;
use rustc_middle::ty::TyS;
use rustc_middle::mir::{BasicBlock, Location};
use rustc_target::spec::Target;
use std::collections::HashMap;

use crate::wasmir::{WasmIR, WasmIRInstruction, WasmIRType};

/// Cranelift codegen backend for WasmRust
pub struct WasmRustCraneliftBackend {
    /// Target ISA for code generation
    isa: Box<dyn TargetIsa>,
    /// WasmRust-specific optimization flags
    optimization_flags: WasmRustOptimizationFlags,
    /// Function compilation cache
    function_cache: HashMap<u64, Vec<u8>>,
    /// Compilation statistics
    stats: CompilationStats,
}

/// WasmRust-specific optimization flags
#[derive(Debug, Clone)]
pub struct WasmRustOptimizationFlags {
    /// Enable thin monomorphization for code deduplication
    pub thin_monomorphization: bool,
    /// Enable streaming layout optimization
    pub streaming_layout: bool,
    /// Enable WASM-specific optimizations
    pub wasm_optimizations: bool,
    /// Enable zero-cost abstractions
    pub zero_cost_abstractions: bool,
}

impl Default for WasmRustOptimizationFlags {
    fn default() -> Self {
        Self {
            thin_monomorphization: true,
            streaming_layout: true,
            wasm_optimizations: true,
            zero_cost_abstractions: true,
        }
    }
}

/// Compilation statistics for performance monitoring
#[derive(Debug, Default)]
pub struct CompilationStats {
    pub functions_compiled: usize,
    pub instructions_generated: usize,
    pub optimization_passes: usize,
    pub compilation_time_ms: u64,
}

impl WasmRustCraneliftBackend {
    /// Creates a new Cranelift backend for WasmRust
    pub fn new(target: Target) -> Result<Self, CodegenError> {
        let isa = create_target_isa(&target)?;
        let optimization_flags = WasmRustOptimizationFlags::default();
        
        Ok(Self {
            isa,
            optimization_flags,
            function_cache: HashMap::new(),
            stats: CompilationStats::default(),
        })
    }

    /// Compiles a WasmIR function to machine code
    pub fn compile_function(
        &mut self,
        wasmir_func: &WasmIR,
        function_name: &str,
    ) -> Result<Vec<u8>, CodegenError> {
        let start_time = std::time::Instant::now();

        // Convert WasmIR to Cranelift IR
        let mut sig = self.convert_signature(&wasmir_func.signature)?;
        let mut func = self.convert_function_body(wasmir_func)?;
        
        // Apply WasmRust-specific optimizations
        self.apply_optimizations(&mut func)?;
        
        // Compile to machine code
        let mut code_gen_context = CodegenContext::new();
        let compiled = code_gen_context.compile(&func, &self.isa, &sig)?;

        let code = compiled.code_buffer.to_vec();

        // Update statistics
        self.stats.functions_compiled += 1;
        self.stats.instructions_generated += func.instructions.len();
        self.stats.compilation_time_ms += start_time.elapsed().as_millis() as u64;

        // Cache compiled function
        let function_hash = self.hash_function(wasmir_func);
        self.function_cache.insert(function_hash, code.clone());

        Ok(code)
    }

    /// Compiles multiple functions with optimizations
    pub fn compile_functions(
        &mut self,
        functions: &[WasmIR],
    ) -> Result<HashMap<String, Vec<u8>>, CodegenError> {
        let mut results = HashMap::new();
        
        for (i, wasmir_func) in functions.iter().enumerate() {
            let function_name = format!("function_{}", i);
            let code = self.compile_function(wasmir_func, &function_name)?;
            results.insert(function_name, code);
        }

        Ok(results)
    }

    /// Gets compilation statistics
    pub fn get_stats(&self) -> &CompilationStats {
        &self.stats
    }

    /// Clears compilation statistics
    pub fn clear_stats(&mut self) {
        self.stats = CompilationStats::default();
    }

    /// Converts WasmIR signature to Cranelift signature
    fn convert_signature(&self, wasmir_sig: &wasmir::Signature) -> Result<Signature, CodegenError> {
        let mut signature = Signature::new();

        // Convert parameters
        for param in &wasmir_sig.params {
            let cranelift_param = self.convert_type(param)?;
            signature.params.push(AbiParam::new(cranelift_param));
        }

        // Convert return type
        if let Some(ret_type) = &wasmir_sig.returns {
            let cranelift_ret = self.convert_type(ret_type)?;
            signature.returns.push(AbiParam::new(cranelift_ret));
        }

        Ok(signature)
    }

    /// Converts WasmIR function body to Cranelift IR
    fn convert_function_body(&self, wasmir_func: &WasmIR) -> Result<Function, CodegenError> {
        let mut func = Function::with_name_signature(
            wasmir_func.name.clone(),
            self.convert_signature(&wasmir_func.signature)?,
        );

        let mut builder = FunctionBuilder::new(&mut func, &mut self.isa);

        // Convert basic blocks
        for (bb_id, bb) in wasmir_func.basic_blocks.iter().enumerate() {
            let block = builder.create_block();
            builder.switch_to_block(block);

            // Convert instructions in this basic block
            for instruction in &bb.instructions {
                self.convert_instruction(&mut builder, instruction)?;
            }
        }

        // Add terminator for the last block
        if let Some(last_bb) = wasmir_func.basic_blocks.last() {
            self.add_block_terminator(&mut builder, &last_bb.terminator)?;
        }

        builder.finalize()?;
        Ok(func)
    }

    /// Converts a WasmIR instruction to Cranelift IR
    fn convert_instruction(
        &self,
        builder: &mut FunctionBuilder,
        instruction: &WasmIRInstruction,
    ) -> Result<Option<Variable>, CodegenError> {
        match instruction {
            WasmIRInstruction::LocalGet { index } => {
                let local = builder.use_var(*index as usize)?;
                Ok(Some(local))
            }
            WasmIRInstruction::LocalSet { index, value } => {
                let local = builder.use_var(*index as usize)?;
                let converted_value = self.convert_operand(builder, value)?;
                builder.def_var(local, converted_value);
                Ok(None)
            }
            WasmIRInstruction::BinaryOp { op, left, right } => {
                let left_var = self.convert_operand(builder, left)?;
                let right_var = self.convert_operand(builder, right)?;
                let result = match op {
                    wasmir::BinaryOp::Add => builder.ins().iadd(left_var, right_var),
                    wasmir::BinaryOp::Sub => builder.ins().isub(left_var, right_var),
                    wasmir::BinaryOp::Mul => builder.ins().imul(left_var, right_var),
                    wasmir::BinaryOp::Div => builder.ins().sdiv(left_var, right_var),
                    wasmir::BinaryOp::Mod => builder.ins().srem(left_var, right_var),
                };
                Ok(Some(result))
            }
            WasmIRInstruction::Call { func_ref, args } => {
                let mut cranelift_args = Vec::new();
                for arg in args {
                    cranelift_args.push(self.convert_operand(builder, arg)?);
                }
                let result = builder.ins().call(*func_ref, &cranelift_args);
                Ok(Some(result))
            }
            WasmIRInstruction::Return { value } => {
                if let Some(val) = value {
                    let converted_val = self.convert_operand(builder, val)?;
                    builder.ins().return_(&[converted_val]);
                } else {
                    builder.ins().return_(&[]);
                }
                Ok(None)
            }
            WasmIRInstruction::Branch { condition, then_block, else_block } => {
                let cond_var = self.convert_operand(builder, condition)?;
                builder.ins().brif(cond_var, *then_block, *else_block);
                Ok(None)
            }
            WasmIRInstruction::MemoryLoad { address, ty } => {
                let addr_var = self.convert_operand(builder, address)?;
                let mem_ty = self.convert_type(ty)?;
                let result = builder.ins().load(mem_ty, cranelift::MemFlags::new(), addr_var, 0);
                Ok(Some(result))
            }
            WasmIRInstruction::MemoryStore { address, value, ty } => {
                let addr_var = self.convert_operand(builder, address)?;
                let value_var = self.convert_operand(builder, value)?;
                let mem_ty = self.convert_type(ty)?;
                builder.ins().store(mem_ty, cranelift::MemFlags::new(), value_var, addr_var, 0);
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Converts a WasmIR operand to Cranelift variable
    fn convert_operand(
        &self,
        builder: &mut FunctionBuilder,
        operand: &wasmir::Operand,
    ) -> Result<Variable, CodegenError> {
        match operand {
            wasmir::Operand::Local(index) => {
                builder.use_var(*index as usize)
            }
            wasmir::Operand::Constant(value) => {
                let const_val = self.convert_constant(value)?;
                Ok(builder.ins().iconst(const_val))
            }
            wasmir::Operand::Global(global_index) => {
                // Global variables need special handling in WASM
                Err(CodegenError::Unsupported("Global variables not yet implemented"))
            }
        }
    }

    /// Converts a WasmIR type to Cranelift type
    fn convert_type(&self, wasmir_ty: &WasmIRType) -> Result<Type, CodegenError> {
        match wasmir_ty {
            WasmIRType::I32 => Ok(types::I32),
            WasmIRType::I64 => Ok(types::I64),
            WasmIRType::F32 => Ok(types::F32),
            WasmIRType::F64 => Ok(types::F64),
            WasmIRType::Ref(ty) => {
                // Handle reference types for ExternRef and FuncRef
                match ty.as_str() {
                    "externref" => Ok(types::R32), // Handle as i32 for now
                    "funcref" => Ok(types::R32), // Handle as i32 for now
                    _ => Err(CodegenError::Unsupported("Unknown reference type")),
                }
            }
            _ => Err(CodegenError::Unsupported("Unsupported type")),
        }
    }

    /// Converts a constant value to Cranelift-compatible value
    fn convert_constant(&self, value: &wasmir::Constant) -> Result<i64, CodegenError> {
        match value {
            wasmir::Constant::I32(v) => Ok(*v as i64),
            wasmir::Constant::I64(v) => Ok(*v),
            wasmir::Constant::F32(v) => Ok(v.to_bits() as i64),
            wasmir::Constant::F64(v) => Ok(v.to_bits()),
            _ => Err(CodegenError::Unsupported("Unsupported constant type")),
        }
    }

    /// Applies WasmRust-specific optimizations to the function
    fn apply_optimizations(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        if self.optimization_flags.thin_monomorphization {
            self.apply_thin_monomorphization(func)?;
        }

        if self.optimization_flags.streaming_layout {
            self.apply_streaming_layout(func)?;
        }

        if self.optimization_flags.wasm_optimizations {
            self.apply_wasm_optimizations(func)?;
        }

        self.stats.optimization_passes += 1;
        Ok(())
    }

    /// Applies thin monomorphization to reduce code duplication
    fn apply_thin_monomorphization(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        // Implementation for thin monomorphization
        // This would analyze generic functions and create specialized versions
        // for common monomorphic instantiations
        
        // For now, placeholder implementation
        Ok(())
    }

    /// Applies streaming layout optimization for fast WASM instantiation
    fn apply_streaming_layout(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        // Implementation for streaming layout optimization
        // This would arrange code layout for optimal streaming
        
        // For now, placeholder implementation
        Ok(())
    }

    /// Applies WASM-specific optimizations
    fn apply_wasm_optimizations(&mut self, func: &mut Function) -> Result<(), CodegenError> {
        // Implementation of WASM-specific optimizations
        // This would include optimizations like:
        // - Zero-cost abstractions
        // - WASM instruction selection
        // - Memory access pattern optimization
        
        // For now, placeholder implementation
        Ok(())
    }

    /// Adds terminator instruction to a basic block
    fn add_block_terminator(
        &self,
        builder: &mut FunctionBuilder,
        terminator: &wasmir::Terminator,
    ) -> Result<(), CodegenError> {
        match terminator {
            wasmir::Terminator::Return { value } => {
                if let Some(val) = value {
                    let converted_val = self.convert_operand(builder, val)?;
                    builder.ins().return_(&[converted_val]);
                } else {
                    builder.ins().return_(&[]);
                }
            }
            wasmir::Terminator::Branch { condition, target } => {
                let cond_var = self.convert_operand(builder, condition)?;
                builder.ins().brif(cond_var, *target, builder.current_block().unwrap());
            }
            wasmir::Terminator::Switch { value, targets } => {
                let value_var = self.convert_operand(builder, value)?;
                let mut switch_targets = Vec::new();
                for target in targets {
                    switch_targets.push(*target);
                }
                builder.ins().br_table(value_var, &switch_targets);
            }
        }
        Ok(())
    }

    /// Hashes a function for caching purposes
    fn hash_function(&self, wasmir_func: &WasmIR) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        wasmir_func.hash(&mut hasher);
        hasher.finish()
    }
}

/// Creates target ISA for compilation
fn create_target_isa(target: &Target) -> Result<Box<dyn TargetIsa>, CodegenError> {
    use cranelift::isa::lookup_target_isa;
    
    let isa_builder = lookup_target_isa(target.arch.as_str())
        .ok_or_else(|| CodegenError::Unsupported("Unsupported target architecture"))?;
    
    let mut flags_builder = cranelift::settings::builder();
    
    // Configure for WASM target
    flags_builder.enable("enable_probestack").unwrap();
    flags_builder.enable("enable_jump_tables").unwrap();
    flags_builder.set("is_pic", "false").unwrap();
    
    let isa = isa_builder.finish(cranelift::settings::FlagsOrIsa::Isa(isa_builder))
        .map_err(|_| CodegenError::Unsupported("Failed to create ISA"))?;
    
    Ok(isa)
}

/// Code generation errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodegenError {
    /// Unsupported operation or type
    Unsupported(&'static str),
    /// Type conversion error
    TypeConversion(&'static str),
    /// Instruction generation error
    InstructionGeneration(&'static str),
    /// Optimization error
    Optimization(&'static str),
    /// Target configuration error
    TargetConfig(&'static str),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::Unsupported(msg) => write!(f, "Unsupported operation: {}", msg),
            CodegenError::TypeConversion(msg) => write!(f, "Type conversion error: {}", msg),
            CodegenError::InstructionGeneration(msg) => write!(f, "Instruction generation error: {}", msg),
            CodegenError::Optimization(msg) => write!(f, "Optimization error: {}", msg),
            CodegenError::TargetConfig(msg) => write!(f, "Target configuration error: {}", msg),
        }
    }
}

impl std::error::Error for CodegenError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasmir;

    #[test]
    fn test_backend_creation() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let backend = WasmRustCraneliftBackend::new(target);
        assert!(backend.is_ok());
        
        let backend = backend.unwrap();
        assert_eq!(backend.get_stats().functions_compiled, 0);
    }

    #[test]
    fn test_simple_function_compilation() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let mut backend = WasmRustCraneliftBackend::new(target).unwrap();
        
        // Create a simple WasmIR function
        let wasmir_func = wasmir::Function {
            name: "test".to_string(),
            signature: wasmir::Signature {
                params: vec![WasmIRType::I32, WasmIRType::I32],
                returns: Some(WasmIRType::I32),
            },
            basic_blocks: vec![
                wasmir::BasicBlock {
                    instructions: vec![
                        wasmir::Instruction::BinaryOp {
                            op: wasmir::BinaryOp::Add,
                            left: wasmir::Operand::Local(0),
                            right: wasmir::Operand::Local(1),
                        },
                        wasmir::Instruction::Return {
                            value: Some(wasmir::Operand::Local(2)), // This would be result of add
                        }
                    ],
                    terminator: wasmir::Terminator::Return {
                        value: Some(wasmir::Operand::Local(2))
                    },
                }
            ],
        };
        
        let result = backend.compile_function(&wasmir_func, "test");
        assert!(result.is_ok());
        
        let stats = backend.get_stats();
        assert_eq!(stats.functions_compiled, 1);
        assert!(stats.instructions_generated > 0);
    }

    #[test]
    fn test_optimization_flags() {
        let flags = WasmRustOptimizationFlags::default();
        assert!(flags.thin_monomorphization);
        assert!(flags.streaming_layout);
        assert!(flags.wasm_optimizations);
        assert!(flags.zero_cost_abstractions);
    }

    #[test]
    fn test_compilation_stats() {
        let mut stats = CompilationStats::default();
        assert_eq!(stats.functions_compiled, 0);
        
        stats.functions_compiled = 10;
        stats.instructions_generated = 1000;
        stats.optimization_passes = 5;
        stats.compilation_time_ms = 150;
        
        assert_eq!(stats.functions_compiled, 10);
        assert_eq!(stats.instructions_generated, 1000);
        assert_eq!(stats.optimization_passes, 5);
        assert_eq!(stats.compilation_time_ms, 150);
        
        stats.clear_stats();
        assert_eq!(stats.functions_compiled, 0);
    }
}

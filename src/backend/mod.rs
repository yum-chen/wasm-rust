//! Backend implementations for WasmRust
//! 
//! This module provides different codegen backends for WasmRust,
//! each optimized for different use cases and host environments.

pub mod cranelift;
pub mod llvm;

use crate::wasmir::WasmIR;
use std::collections::HashMap;

/// Backend compilation result
#[derive(Debug)]
pub struct CompilationResult {
    /// Compiled machine code
    pub code: Vec<u8>,
    /// Symbol table for linking
    pub symbols: HashMap<String, u64>,
    /// Relocation information
    pub relocations: Vec<Relocation>,
    /// Compilation metadata
    pub metadata: CompilationMetadata,
}

/// Relocation information for linking
#[derive(Debug, Clone)]
pub struct Relocation {
    /// Type of relocation
    pub kind: RelocationKind,
    /// Offset in the code where relocation applies
    pub offset: u32,
    /// Symbol being referenced
    pub symbol: String,
    /// Addend to add to symbol value
    pub addend: i64,
}

/// Types of relocations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    /// Absolute address relocation
    Absolute,
    /// Relative address relocation
    Relative,
    /// Function call relocation
    FunctionCall,
    /// Data access relocation
    DataAccess,
    /// Global variable access
    GlobalAccess,
}

/// Compilation metadata
#[derive(Debug, Clone)]
pub struct CompilationMetadata {
    /// Target triple
    pub target: String,
    /// Optimization level used
    pub optimization_level: OptimizationLevel,
    /// Build profile used
    pub build_profile: BuildProfile,
    /// Compilation timestamp
    pub timestamp: std::time::SystemTime,
}

/// Optimization levels for compilation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationLevel {
    /// No optimizations (debug builds)
    None,
    /// Basic optimizations only
    Basic,
    /// Standard optimizations
    Standard,
    /// Aggressive optimizations
    Aggressive,
    /// Profile-guided optimizations
    PGO,
}

/// Build profiles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    /// Freestanding profile (no stdlib)
    Freestanding,
    /// Development profile (fast compilation)
    Development,
    /// Release profile (maximum optimization)
    Release,
}

/// Backend trait for different codegen implementations
pub trait Backend {
    /// Compiles WasmIR to machine code
    fn compile(&mut self, wasmir: &WasmIR, profile: BuildProfile) -> Result<CompilationResult, BackendError>;
    
    /// Gets supported optimization levels
    fn supported_optimizations(&self) -> Vec<OptimizationLevel>;
    
    /// Gets backend capabilities
    fn capabilities(&self) -> BackendCapabilities;
    
    /// Resets backend state
    fn reset(&mut self);
}

/// Backend capabilities
#[derive(Debug, Clone)]
pub struct BackendCapabilities {
    /// Supports thin monomorphization
    pub thin_monomorphization: bool,
    /// Supports streaming layout optimization
    pub streaming_layout: bool,
    /// Supports profile-guided optimization
    pub pgo_support: bool,
    /// Supports component model codegen
    pub component_model: bool,
    /// Supports WASM-specific optimizations
    pub wasm_optimizations: bool,
    /// Supports linear types
    pub linear_types: bool,
}

/// Backend errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackendError {
    /// Compilation failed
    CompilationFailed(String),
    /// Unsupported operation
    Unsupported(String),
    /// Linking failed
    LinkingFailed(String),
    /// Target not supported
    UnsupportedTarget(String),
    /// Optimization failed
    OptimizationFailed(String),
    /// Resource exhausted
    ResourceExhausted(String),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::CompilationFailed(msg) => write!(f, "Compilation failed: {}", msg),
            BackendError::Unsupported(msg) => write!(f, "Unsupported: {}", msg),
            BackendError::LinkingFailed(msg) => write!(f, "Linking failed: {}", msg),
            BackendError::UnsupportedTarget(msg) => write!(f, "Unsupported target: {}", msg),
            BackendError::OptimizationFailed(msg) => write!(f, "Optimization failed: {}", msg),
            BackendError::ResourceExhausted(msg) => write!(f, "Resource exhausted: {}", msg),
        }
    }
}

impl std::error::Error for BackendError {}

/// Backend factory for creating appropriate backend
pub struct BackendFactory;

impl BackendFactory {
    /// Creates a backend for the specified target and profile
    pub fn create_backend(
        target: &str,
        profile: BuildProfile,
    ) -> Result<Box<dyn Backend>, BackendError> {
        match profile {
            BuildProfile::Development => {
                // Use Cranelift for fast development builds
                let cranelift_backend = crate::backend::cranelift::WasmRustCraneliftBackend::new(
                    rustc_target::spec::Target {
                        arch: target.to_string(),
                        ..Default::default()
                    }
                )?;
                Ok(Box::new(cranelift_backend))
            }
            BuildProfile::Release => {
                // Use LLVM for optimized release builds
                #[cfg(feature = "llvm-backend")]
                {
                    let llvm_backend = crate::backend::llvm::WasmRustLLVMBackend::new(
                        rustc_target::spec::Target {
                            arch: target.to_string(),
                            ..Default::default()
                        }
                    )?;
                    return Ok(Box::new(llvm_backend));
                }
                
                #[cfg(not(feature = "llvm-backend"))]
                {
                    // Fallback to Cranelift if LLVM not available
                    let cranelift_backend = crate::backend::cranelift::WasmRustCraneliftBackend::new(
                        rustc_target::spec::Target {
                            arch: target.to_string(),
                            ..Default::default()
                        }
                    )?;
                    Ok(Box::new(cranelift_backend))
                }
            }
            BuildProfile::Freestanding => {
                // Use Cranelift for freestanding builds (minimal overhead)
                let cranelift_backend = crate::backend::cranelift::WasmRustCraneliftBackend::new(
                    rustc_target::spec::Target {
                        arch: target.to_string(),
                        ..Default::default()
                    }
                )?;
                Ok(Box::new(cranelift_backend))
            }
        }
    }

    /// Lists available backends
    pub fn available_backends() -> Vec<&'static str> {
        let mut backends = vec!["cranelift"];
        
        #[cfg(feature = "llvm-backend")]
        {
            backends.push("llvm");
        }
        
        backends
    }

    /// Gets recommended backend for target and profile
    pub fn recommend_backend(
        target: &str,
        profile: BuildProfile,
    ) -> Option<&'static str> {
        match (target, profile) {
            // Recommendation logic based on target and profile
            ("wasm32", BuildProfile::Development) => Some("cranelift"),
            ("wasm32", BuildProfile::Release) => Some("cranelift"), // LLVM if available
            ("wasm32", BuildProfile::Freestanding) => Some("cranelift"),
            _ => None,
        }
    }

    /// Validates backend compatibility
    pub fn validate_backend(backend: &dyn Backend) -> Result<(), BackendError> {
        let capabilities = backend.capabilities();
        
        // Check required capabilities for WasmRust
        if !capabilities.wasm_optimizations {
            return Err(BackendError::Unsupported(
                "Backend must support WASM optimizations".to_string()
            ));
        }
        
        if !capabilities.thin_monomorphization {
            return Err(BackendError::Unsupported(
                "Backend must support thin monomorphization".to_string()
            ));
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_factory_creation() {
        let backend = BackendFactory::create_backend("wasm32", BuildProfile::Development);
        assert!(backend.is_ok());
    }

    #[test]
    fn test_available_backends() {
        let backends = BackendFactory::available_backends();
        assert!(backends.contains(&"cranelift"));
        
        #[cfg(feature = "llvm-backend")]
        {
            assert!(backends.contains(&"llvm"));
        }
    }

    #[test]
    fn test_backend_recommendations() {
        let recommended = BackendFactory::recommend_backend("wasm32", BuildProfile::Development);
        assert_eq!(recommended, Some("cranelift"));
        
        let recommended = BackendFactory::recommend_backend("wasm32", BuildProfile::Release);
        assert_eq!(recommended, Some("cranelift"));
        
        let recommended = BackendFactory::recommend_backend("wasm32", BuildProfile::Freestanding);
        assert_eq!(recommended, Some("cranelift"));
    }

    #[test]
    fn test_compilation_result() {
        let result = CompilationResult {
            code: vec![0x01, 0x02, 0x03],
            symbols: HashMap::new(),
            relocations: Vec::new(),
            metadata: CompilationMetadata {
                target: "wasm32".to_string(),
                optimization_level: OptimizationLevel::Standard,
                build_profile: BuildProfile::Release,
                timestamp: std::time::SystemTime::UNIX_EPOCH,
            },
        };
        
        assert_eq!(result.code, vec![0x01, 0x02, 0x03]);
        assert!(result.symbols.is_empty());
        assert!(result.relocations.is_empty());
        assert_eq!(result.metadata.target, "wasm32");
        assert_eq!(result.metadata.build_profile, BuildProfile::Release);
    }

    #[test]
    fn test_relocation() {
        let relocation = Relocation {
            kind: RelocationKind::FunctionCall,
            offset: 42,
            symbol: "test_function".to_string(),
            addend: 0,
        };
        
        assert_eq!(relocation.kind, RelocationKind::FunctionCall);
        assert_eq!(relocation.offset, 42);
        assert_eq!(relocation.symbol, "test_function");
        assert_eq!(relocation.addend, 0);
    }

    #[test]
    fn test_backend_capabilities() {
        let capabilities = BackendCapabilities {
            thin_monomorphization: true,
            streaming_layout: true,
            pgo_support: true,
            component_model: true,
            wasm_optimizations: true,
            linear_types: true,
        };
        
        assert!(capabilities.thin_monomorphization);
        assert!(capabilities.streaming_layout);
        assert!(capabilities.pgo_support);
        assert!(capabilities.component_model);
        assert!(capabilities.wasm_optimizations);
        assert!(capabilities.linear_types);
    }
}

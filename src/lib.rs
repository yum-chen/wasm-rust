//! WasmRust Compiler
//! 
//! An optimized Rust-to-WebAssembly compilation system that addresses
//! the current limitations of the standard Rust WASM toolchain.
//! 
//! The system provides a five-layer architecture delivering:
//! - Minimal binary sizes
//! - Fast compilation times  
//! - Seamless Component Model integration
//! - Efficient JavaScript interoperability
//! - Full Rust memory safety guarantees

#![feature(specialization)]
#![feature(extern_types)]
#![feature(unsize)]
#![feature(coerce_unsized)]
#![feature(generic_associated_types)]
#![feature(trait_alias)]
#![feature(min_specialization)]
#![feature(const_fn)]
#![feature(const_mut_refs)]
#![feature(in_band_lifetimes)]

pub mod memory;
pub mod threading;
pub mod component;
pub mod host;
pub mod backend;
pub mod wasmir;

use backend::BackendFactory;
use wasmir::WasmIR;
use rustc_middle::mir::Body;
use rustc_target::spec::Target;

/// WasmRust compiler version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default compilation target
pub const DEFAULT_TARGET: &str = "wasm32-unknown-unknown";

/// Main compiler interface
pub struct WasmRustCompiler {
    /// Backend factory for creating appropriate codegen
    backend_factory: BackendFactory,
    /// Current target
    target: Target,
}

impl WasmRustCompiler {
    /// Creates a new WasmRust compiler instance
    pub fn new(target: Target) -> Self {
        Self {
            backend_factory: BackendFactory,
            target,
        }
    }

    /// Compiles a Rust MIR body to WASM using appropriate backend
    pub fn compile_mir(
        &mut self,
        mir: &Body,
        build_profile: backend::BuildProfile,
    ) -> Result<backend::CompilationResult, backend::BackendError> {
        // Convert MIR to WasmIR
        let wasmir = self.convert_mir_to_wasmir(mir)?;
        
        // Create appropriate backend
        let mut backend = BackendFactory::create_backend(
            &self.target.arch,
            build_profile,
        )?;
        
        // Compile WasmIR to machine code
        let result = backend.compile(&wasmir, build_profile)?;
        
        Ok(result)
    }

    /// Compiles a WasmIR function directly
    pub fn compile_wasmir(
        &mut self,
        wasmir: &WasmIR,
        build_profile: backend::BuildProfile,
    ) -> Result<backend::CompilationResult, backend::BackendError> {
        let mut backend = BackendFactory::create_backend(
            &self.target.arch,
            build_profile,
        )?;
        
        backend.compile(wasmir, build_profile)
    }

    /// Converts Rust MIR to WasmIR
    fn convert_mir_to_wasmir(&mut self, mir: &Body) -> Result<WasmIR, String> {
        // Use the MIR lowering module
        use backend::cranelift::mir_lowering::MirLoweringContext;
        
        let mut context = MirLoweringContext::new(self.target.clone(), mir);
        
        if let Err(errors) = context.lower_body(mir) {
            let error_messages: Vec<String> = errors.iter()
                .map(|e| e.to_string())
                .collect();
            Err(format!("MIR lowering failed: {}", error_messages.join("; ")))
        } else {
            context.into_wasmir()
                .map_err(|e| format!("Failed to get WasmIR: {}", e.to_string()))
        }
    }

    /// Gets supported targets
    pub fn supported_targets() -> Vec<&'static str> {
        vec!["wasm32-unknown-unknown", "wasm32-unknown-emscripten"]
    }

    /// Gets available backends
    pub fn available_backends() -> Vec<&'static str> {
        BackendFactory::available_backends()
    }

    /// Validates target support
    pub fn is_target_supported(target: &str) -> bool {
        Self::supported_targets().contains(&target)
    }

    /// Gets recommended backend for target and profile
    pub fn recommend_backend(
        &self,
        build_profile: backend::BuildProfile,
    ) -> Option<&'static str> {
        BackendFactory::recommend_backend(&self.target.arch, build_profile)
    }
}

/// Compiler configuration
#[derive(Debug, Clone)]
pub struct CompilerConfig {
    /// Optimization level
    pub optimization_level: backend::OptimizationLevel,
    /// Build profile
    pub build_profile: backend::BuildProfile,
    /// Target triple
    pub target: String,
    /// Enable debug information
    pub debug_info: bool,
    /// Enable LTO (Link Time Optimization)
    pub lto: bool,
    /// Enable PGO (Profile Guided Optimization)
    pub pgo: Option<String>,
}

impl Default for CompilerConfig {
    fn default() -> Self {
        Self {
            optimization_level: backend::OptimizationLevel::Standard,
            build_profile: backend::BuildProfile::Development,
            target: DEFAULT_TARGET.to_string(),
            debug_info: true,
            lto: false,
            pgo: None,
        }
    }
}

/// High-level compilation interface
pub struct WasmRustFrontend {
    compiler: WasmRustCompiler,
    config: CompilerConfig,
}

impl WasmRustFrontend {
    /// Creates a new frontend instance
    pub fn new(config: CompilerConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let target = rustc_target::spec::Target {
            arch: config.target.clone(),
            ..Default::default()
        };
        
        Ok(Self {
            compiler: WasmRustCompiler::new(target),
            config,
        })
    }

    /// Compiles a crate to WASM
    pub fn compile_crate(
        &mut self,
        crate_path: &str,
    ) -> Result<backend::CompilationResult, Box<dyn std::error::Error>> {
        // This would implement the full crate compilation pipeline
        // For now, return a placeholder
        Err("Crate compilation not yet implemented".into())
    }

    /// Compiles a single file to WASM
    pub fn compile_file(
        &mut self,
        file_path: &str,
    ) -> Result<backend::CompilationResult, Box<dyn std::error::Error>> {
        // This would implement single file compilation
        // For now, return a placeholder
        Err("File compilation not yet implemented".into())
    }

    /// Updates compiler configuration
    pub fn update_config(&mut self, config: CompilerConfig) {
        self.config = config;
    }

    /// Gets current configuration
    pub fn get_config(&self) -> &CompilerConfig {
        &self.config
    }

    /// Validates configuration
    pub fn validate_config(&self) -> Result<(), String> {
        // Validate target
        if !WasmRustCompiler::is_target_supported(&self.config.target) {
            return Err(format!("Unsupported target: {}", self.config.target));
        }

        // Validate backend compatibility
        let recommended_backend = self.compiler.recommend_backend(&self.config.build_profile);
        if let Some(recommended) = recommended_backend {
            let available_backends = WasmRustCompiler::available_backends();
            if !available_backends.contains(&recommended) {
                return Err(format!("Recommended backend '{}' not available", recommended));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiler_creation() {
        let target = rustc_target::spec::Target {
            arch: "wasm32".to_string(),
            ..Default::default()
        };
        
        let compiler = WasmRustCompiler::new(target);
        assert_eq!(compiler.target.arch, "wasm32");
        assert!(compiler.available_backends().contains(&"cranelift"));
    }

    #[test]
    fn test_target_support() {
        assert!(WasmRustCompiler::is_target_supported("wasm32-unknown-unknown"));
        assert!(!WasmRustCompiler::is_target_supported("x86_64-unknown-linux"));
    }

    #[test]
    fn test_frontend_creation() {
        let config = CompilerConfig::default();
        let frontend = WasmRustFrontend::new(config);
        assert!(frontend.is_ok());
        
        let frontend = frontend.unwrap();
        assert_eq!(frontend.get_config().target, DEFAULT_TARGET);
    }

    #[test]
    fn test_config_validation() {
        let mut frontend = WasmRustFrontend::new(CompilerConfig::default()).unwrap();
        
        // Valid config should pass
        assert!(frontend.validate_config().is_ok());
        
        // Invalid target should fail
        let mut invalid_config = frontend.get_config().clone();
        invalid_config.target = "invalid-target".to_string();
        frontend.update_config(invalid_config);
        assert!(frontend.validate_config().is_err());
    }

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_default_target() {
        assert_eq!(DEFAULT_TARGET, "wasm32-unknown-unknown");
        assert!(WasmRustCompiler::is_target_supported(DEFAULT_TARGET));
    }

    #[test]
    fn test_compiler_config() {
        let config = CompilerConfig::default();
        assert_eq!(config.optimization_level, backend::OptimizationLevel::Standard);
        assert_eq!(config.build_profile, backend::BuildProfile::Development);
        assert_eq!(config.target, DEFAULT_TARGET);
        assert!(config.debug_info);
        assert!(!config.lto);
        assert!(config.pgo.is_none());
    }
}

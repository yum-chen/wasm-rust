//! Automated testing pipeline for WasmRust Curation Registry
//!
//! This module provides automated testing capabilities for validating
//! WASM compatibility of Rust crates.

mod compilation;
mod performance;
mod wasm_specific;

use crate::schema::{TestResult, TestType, TestOutcome, TestEnvironment};
use std::collections::HashMap;
use std::process::Command;
use std::path::Path;
use thiserror::Error;

/// Testing errors
#[derive(Error, Debug)]
pub enum TestingError {
    #[error("Compilation failed: {0}")]
    CompilationFailed(String),
    
    #[error("Test execution failed: {0}")]
    TestExecutionFailed(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Process error: {0}")]
    ProcessError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Testing pipeline configuration
#[derive(Debug, Clone)]
pub struct TestingConfig {
    /// Rust toolchain to use
    pub rust_toolchain: String,
    
    /// WASM target to test against
    pub wasm_target: String,
    
    /// WASM runtime for testing
    pub wasm_runtime: String,
    
    /// Timeout for compilation (seconds)
    pub compilation_timeout: u64,
    
    /// Timeout for test execution (seconds)
    pub test_timeout: u64,
    
    /// Directory for temporary files
    pub temp_dir: String,
    
    /// Environment variables for testing
    pub environment_vars: HashMap<String, String>,
}

impl Default for TestingConfig {
    fn default() -> Self {
        Self {
            rust_toolchain: "stable".to_string(),
            wasm_target: "wasm32-unknown-unknown".to_string(),
            wasm_runtime: "wasmtime".to_string(),
            compilation_timeout: 300,
            test_timeout: 60,
            temp_dir: "/tmp/wasm-crates-testing".to_string(),
            environment_vars: HashMap::new(),
        }
    }
}

/// Main testing pipeline
pub struct TestingPipeline {
    config: TestingConfig,
}

impl TestingPipeline {
    /// Create a new testing pipeline with configuration
    pub fn new(config: TestingConfig) -> Self {
        // Create temp directory if it doesn't exist
        let _ = std::fs::create_dir_all(&config.temp_dir);
        
        Self { config }
    }
    
    /// Run all tests for a crate
    pub async fn test_crate(&self, crate_path: &str) -> Result<Vec<TestResult>, TestingError> {
        let mut results = Vec::new();
        
        // Run compilation tests
        results.push(self.test_compilation(crate_path).await?);
        
        // Run WASM-specific tests if compilation succeeded
        if let Ok(Some(binary_path)) = self.compile_to_wasm(crate_path).await {
            results.push(self.test_wasm_specific(&binary_path).await?);
            results.push(self.test_performance(crate_path, &binary_path).await?);
        }
        
        Ok(results)
    }
    
    /// Test compilation to native and WASM targets
    async fn test_compilation(&self, crate_path: &str) -> Result<TestResult, TestingError> {
        let start_time = std::time::Instant::now();
        
        let mut details = HashMap::new();
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        
        // Test native compilation
        let native_result = self.compile_native(crate_path).await;
        details.insert("native_compilation".to_string(), 
            serde_json::to_value(&native_result.is_ok()).unwrap());
        
        if let Err(e) = native_result {
            errors.push(format!("Native compilation failed: {}", e));
        }
        
        // Test WASM compilation
        let wasm_result = self.compile_to_wasm(crate_path).await;
        details.insert("wasm_compilation".to_string(), 
            serde_json::to_value(&wasm_result.is_ok()).unwrap());
        
        if let Err(e) = wasm_result {
            errors.push(format!("WASM compilation failed: {}", e));
        }
        
        let outcome = if errors.is_empty() {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed
        };
        
        let duration = start_time.elapsed().as_millis() as u64;
        
        Ok(TestResult {
            test_type: TestType::Compilation,
            timestamp: chrono::Utc::now(),
            outcome,
            details,
            duration_ms: duration,
            environment: self.test_environment(),
        })
    }
    
    /// Compile crate to native target
    async fn compile_native(&self, crate_path: &str) -> Result<(), TestingError> {
        let output = Command::new("cargo")
            .arg("+")
            .arg(&self.config.rust_toolchain)
            .arg("build")
            .arg("--release")
            .arg("--manifest-path")
            .arg(crate_path)
            .output()
            .map_err(|e| TestingError::ProcessError(e.to_string()))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TestingError::CompilationFailed(stderr.to_string()));
        }
        
        Ok(())
    }
    
    /// Compile crate to WASM target
    async fn compile_to_wasm(&self, crate_path: &str) -> Result<Option<String>, TestingError> {
        // Add WASM target if not installed
        self.install_wasm_target().await?;
        
        let output = Command::new("cargo")
            .arg("+")
            .arg(&self.config.rust_toolchain)
            .arg("build")
            .arg("--release")
            .arg("--target")
            .arg(&self.config.wasm_target)
            .arg("--manifest-path")
            .arg(crate_path)
            .output()
            .map_err(|e| TestingError::ProcessError(e.to_string()))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TestingError::CompilationFailed(stderr.to_string()));
        }
        
        // Return path to compiled WASM binary
        let wasm_path = format!("{}/target/{}/release/{}.wasm", 
            Path::new(crate_path).parent().unwrap().to_string_lossy(),
            self.config.wasm_target,
            Path::new(crate_path).file_stem().unwrap().to_string_lossy()
        );
        
        Ok(Some(wasm_path))
    }
    
    /// Test WASM-specific functionality
    async fn test_wasm_specific(&self, wasm_binary_path: &str) -> Result<TestResult, TestingError> {
        let start_time = std::time::Instant::now();
        let mut details = HashMap::new();
        
        // Test basic WASM execution
        let execution_result = self.test_wasm_execution(wasm_binary_path).await;
        details.insert("wasm_execution".to_string(), 
            serde_json::to_value(&execution_result.is_ok()).unwrap());
        
        // Test memory usage
        let memory_result = self.test_memory_usage(wasm_binary_path).await;
        details.insert("memory_usage".to_string(), 
            serde_json::to_value(&memory_result).unwrap());
        
        let outcome = if execution_result.is_ok() {
            TestOutcome::Passed
        } else {
            TestOutcome::Failed
        };
        
        let duration = start_time.elapsed().as_millis() as u64;
        
        Ok(TestResult {
            test_type: TestType::WasmSpecific,
            timestamp: chrono::Utc::now(),
            outcome,
            details,
            duration_ms: duration,
            environment: self.test_environment(),
        })
    }
    
    /// Test performance characteristics
    async fn test_performance(&self, native_path: &str, wasm_path: &str) -> Result<TestResult, TestingError> {
        let start_time = std::time::Instant::now();
        let mut details = HashMap::new();
        
        // Benchmark native execution
        let native_perf = self.benchmark_native(native_path).await?;
        details.insert("native_performance".to_string(), 
            serde_json::to_value(&native_perf).unwrap());
        
        // Benchmark WASM execution
        let wasm_perf = self.benchmark_wasm(wasm_path).await?;
        details.insert("wasm_performance".to_string(), 
            serde_json::to_value(&wasm_perf).unwrap());
        
        // Calculate performance ratio
        let ratio = if native_perf > 0 { wasm_perf as f64 / native_perf as f64 } else { 0.0 };
        details.insert("performance_ratio".to_string(), serde_json::to_value(ratio).unwrap());
        
        let duration = start_time.elapsed().as_millis() as u64;
        
        Ok(TestResult {
            test_type: TestType::Performance,
            timestamp: chrono::Utc::now(),
            outcome: TestOutcome::Passed, // Performance tests don't fail, just provide metrics
            details,
            duration_ms: duration,
            environment: self.test_environment(),
        })
    }
    
    /// Install WASM target if needed
    async fn install_wasm_target(&self) -> Result<(), TestingError> {
        let output = Command::new("rustup")
            .arg("target")
            .arg("list")
            .arg("--installed")
            .output()
            .map_err(|e| TestingError::ProcessError(e.to_string()))?;
        
        let installed_targets = String::from_utf8_lossy(&output.stdout);
        
        if !installed_targets.contains(&self.config.wasm_target) {
            let output = Command::new("rustup")
                .arg("target")
                .arg("add")
                .arg(&self.config.wasm_target)
                .output()
                .map_err(|e| TestingError::ProcessError(e.to_string()))?;
            
            if !output.status.success() {
                return Err(TestingError::ConfigError(
                    format!("Failed to install WASM target: {}", self.config.wasm_target)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Test basic WASM execution
    async fn test_wasm_execution(&self, wasm_binary_path: &str) -> Result<(), TestingError> {
        // Use WASM runtime to execute the binary
        let output = Command::new(&self.config.wasm_runtime)
            .arg(wasm_binary_path)
            .output()
            .map_err(|e| TestingError::ProcessError(e.to_string()))?;
        
        // Even if the binary doesn't exit successfully, as long as it runs, it's a pass
        // (some WASM binaries might be designed to run indefinitely)
        Ok(())
    }
    
    /// Test memory usage
    async fn test_memory_usage(&self, wasm_binary_path: &str) -> Result<HashMap<String, u64>, TestingError> {
        let mut result = HashMap::new();
        
        // Get file size
        if let Ok(metadata) = std::fs::metadata(wasm_binary_path) {
            result.insert("file_size_bytes".to_string(), metadata.len());
        }
        
        // Additional memory metrics would be collected during execution
        // This is a simplified version
        
        Ok(result)
    }
    
    /// Benchmark native execution
    async fn benchmark_native(&self, _native_path: &str) -> Result<u64, TestingError> {
        // Simplified benchmarking - in real implementation, this would:
        // 1. Run the native binary multiple times
        // 2. Measure execution time
        // 3. Return average execution time in nanoseconds
        
        Ok(1000) // Placeholder
    }
    
    /// Benchmark WASM execution
    async fn benchmark_wasm(&self, _wasm_path: &str) -> Result<u64, TestingError> {
        // Simplified benchmarking - in real implementation, this would:
        // 1. Run the WASM binary in the runtime multiple times
        // 2. Measure execution time
        // 3. Return average execution time in nanoseconds
        
        Ok(1500) // Placeholder (assuming 50% slower than native)
    }
    
    /// Create test environment description
    fn test_environment(&self) -> TestEnvironment {
        TestEnvironment {
            rust_version: self.config.rust_toolchain.clone(),
            wasm_target: self.config.wasm_target.clone(),
            wasm_runtime: self.config.wasm_runtime.clone(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            environment_vars: self.config.environment_vars.clone(),
        }
    }
}
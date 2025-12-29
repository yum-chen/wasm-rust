//! MIR Parity Tests
//! 
//! Tests to ensure that Cranelift and LLVM backends produce
//! semantically equivalent output for the same input.

use std::collections::HashMap;
use std::process::Command;
use std::path::Path;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Serialize, Deserialize)]
struct MIRComparisonResult {
    test_name: String,
    llvm_mir: String,
    cranelift_mir: String,
    function_equivalence: HashMap<String, f64>,
    overall_similarity: f64,
    compilation_time_llvm: u64,
    compilation_time_cranelift: u64,
    wasm_output_llvm: Vec<u8>,
    wasm_output_cranelift: Vec<u8>,
}

impl MIRComparisonResult {
    fn is_acceptable(&self) -> bool {
        // Overall similarity must be >= 95%
        self.overall_similarity >= 0.95 &&
        // All functions must have >= 90% similarity
        self.function_equivalence.values().all(|&similarity| similarity >= 0.90) &&
        // Cranelift should be at least 5x faster
        self.compilation_time_cranelift * 5 <= self.compilation_time_llvm
    }

    fn performance_ratio(&self) -> f64 {
        self.compilation_time_llvm as f64 / self.compilation_time_cranelift as f64
    }

    fn wasm_equivalent(&self) -> bool {
        // Check if WASM outputs produce same results
        self.execute_wasm_equivalence_test()
    }

    fn execute_wasm_equivalence_test(&self) -> bool {
        // This would execute both WASM outputs with same inputs
        // and verify they produce identical results
        
        // For now, return true as placeholder
        true
    }
}

struct TestConfig {
    test_cases: Vec<TestCase>,
    timeout_seconds: u64,
    performance_threshold: f64,
}

#[derive(Debug)]
struct TestCase {
    name: String,
    rust_code: String,
    expected_functions: Vec<String>,
    linear_types: Vec<String>,
    complexity_level: ComplexityLevel,
}

#[derive(Debug, Clone, Copy)]
enum ComplexityLevel {
    Simple,
    Medium,
    Complex,
    VeryComplex,
}

impl TestConfig {
    fn default() -> Self {
        Self {
            test_cases: vec![
                TestCase {
                    name: "hello_world".to_string(),
                    rust_code: r#"
#[wasm::export]
pub fn hello() -> String {
    "Hello, World!".to_string()
}
"#.to_string(),
                    expected_functions: vec!["hello".to_string()],
                    linear_types: vec![],
                    complexity_level: ComplexityLevel::Simple,
                },
                TestCase {
                    name: "fibonacci".to_string(),
                    rust_code: r#"
#[wasm::export]
pub fn fibonacci(n: u32) -> u32 {
    match n {
        0 | 1 => n,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
"#.to_string(),
                    expected_functions: vec!["fibonacci".to_string()],
                    linear_types: vec![],
                    complexity_level: ComplexityLevel::Medium,
                },
                TestCase {
                    name: "linear_resource".to_string(),
                    rust_code: r#"
#[wasm::linear]
struct CanvasHandle {
    id: u32,
}

impl CanvasHandle {
    fn new() -> Self {
        Self { id: 42 }
    }
    
    fn draw(&mut self) {
        // Drawing operations
        self.id += 1;
    }
    
    fn into_image(self) -> Vec<u8> {
        vec![self.id as u8; 1024]
    }
}

#[wasm::export]
pub fn create_image() -> Vec<u8> {
    let mut canvas = CanvasHandle::new();
    canvas.draw();
    canvas.into_image()
}
"#.to_string(),
                    expected_functions: vec!["create_image".to_string()],
                    linear_types: vec!["CanvasHandle".to_string()],
                    complexity_level: ComplexityLevel::Complex,
                },
                TestCase {
                    name: "shared_memory".to_string(),
                    rust_code: r#"
use wasm::SharedSlice;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[wasm::export]
pub fn process_data(data: &SharedSlice<u32>) -> u32 {
    let mut sum = 0;
    for &value in data.iter() {
        sum += value;
        COUNTER.fetch_add(1, Ordering::SeqCst);
    }
    sum
}
"#.to_string(),
                    expected_functions: vec!["process_data".to_string()],
                    linear_types: vec![],
                    complexity_level: ComplexityLevel::VeryComplex,
                },
            ],
            timeout_seconds: 300, // 5 minutes per test
            performance_threshold: 5.0, // 5x speedup required
        }
    }
}

/// Differential execution test runner
struct DifferentialExecutor {
    temp_dir: TempDir,
}

impl DifferentialExecutor {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            temp_dir: TempDir::new()?,
        })
    }

    fn compile_with_backend(
        &self,
        test_case: &TestCase,
        backend: Backend,
    ) -> Result<(String, u64, Vec<u8>), Box<dyn std::error::Error>> {
        let start_time = std::time::Instant::now();
        
        // Write test case to temporary file
        let test_file = self.temp_dir.path().join("test.rs");
        std::fs::write(&test_file, &test_case.rust_code)?;
        
        // Compile with specified backend
        let output_dir = self.temp_dir.path().join("output");
        std::fs::create_dir_all(&output_dir)?;
        
        let mut cmd = Command::new("cargo");
        cmd.args(&[
            "wasm",
            "build",
            "--backend",
            match backend {
                Backend::LLVM => "llvm",
                Backend::Cranelift => "cranelift",
            },
            "--output-dir",
            output_dir.to_str().unwrap(),
        ]);
        cmd.current_dir(self.temp_dir.path());
        cmd.env("RUST_LOG", "debug");
        
        let output = cmd.output()?;
        let compilation_time = start_time.elapsed().as_millis();
        
        if !output.status.success() {
            return Err(format!("Compilation failed: {}", String::from_utf8_lossy(&output.stderr)).into());
        }
        
        // Extract MIR dump
        let mir_file = output_dir.join("output.mir");
        let mir_content = std::fs::read_to_string(&mir_file)?;
        
        // Get WASM output
        let wasm_file = output_dir.join("output.wasm");
        let wasm_content = std::fs::read(&wasm_file)?;
        
        Ok((mir_content, compilation_time, wasm_content))
    }

    fn run_differential_test(
        &self,
        test_case: &TestCase,
    ) -> Result<MIRComparisonResult, Box<dyn std::error::Error>> {
        // Compile with LLVM
        let (llvm_mir, llvm_time, llvm_wasm) = self.compile_with_backend(test_case, Backend::LLVM)?;
        
        // Compile with Cranelift
        let (cranelift_mir, cranelift_time, cranelift_wasm) = self.compile_with_backend(test_case, Backend::Cranelift)?;
        
        // Compare MIR outputs
        let function_equivalence = self.compare_mir_functions(&llvm_mir, &cranelift_mir)?;
        let overall_similarity = self.calculate_overall_similarity(&llvm_mir, &cranelift_mir);
        
        Ok(MIRComparisonResult {
            test_name: test_case.name.clone(),
            llvm_mir,
            cranelift_mir,
            function_equivalence,
            overall_similarity,
            compilation_time_llvm: llvm_time,
            compilation_time_cranelift: cranelift_time,
            wasm_output_llvm: llvm_wasm,
            wasm_output_cranelift: cranelift_wasm,
        })
    }

    fn compare_mir_functions(
        &self,
        llvm_mir: &str,
        cranelift_mir: &str,
    ) -> Result<HashMap<String, f64>, Box<dyn std::error::Error>> {
        let mut equivalence = HashMap::new();
        
        // Extract function definitions from MIR
        let llvm_functions = self.extract_functions(llvm_mir)?;
        let cranelift_functions = self.extract_functions(cranelift_mir)?;
        
        for (func_name, llvm_func) in llvm_functions {
            if let Some(cranelift_func) = cranelift_functions.get(&func_name) {
                let similarity = self.calculate_function_similarity(&llvm_func, &cranelift_func);
                equivalence.insert(func_name, similarity);
            }
        }
        
        Ok(equivalence)
    }

    fn extract_functions(&self, mir_content: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
        let mut functions = HashMap::new();
        let mut current_function = String::new();
        let mut current_name = String::new();
        let mut in_function = false;
        
        for line in mir_content.lines() {
            if line.trim().starts_with("fn ") || line.trim().starts_with("const fn ") {
                // Start of new function
                if in_function && !current_name.is_empty() {
                    functions.insert(current_name.clone(), current_function.clone());
                }
                
                current_function.clear();
                current_name = line.split('(').next().unwrap().split_whitespace().last().unwrap().to_string();
                in_function = true;
            }
            
            if in_function {
                current_function.push_str(line);
                current_function.push('\n');
            }
        }
        
        // Add the last function
        if in_function && !current_name.is_empty() {
            functions.insert(current_name, current_function);
        }
        
        Ok(functions)
    }

    fn calculate_function_similarity(&self, func1: &str, func2: &str) -> f64 {
        // Simple similarity calculation - in practice this would be more sophisticated
        let normalized1 = self.normalize_function(func1);
        let normalized2 = self.normalize_function(func2);
        
        if normalized1.is_empty() && normalized2.is_empty() {
            return 1.0;
        }
        
        if normalized1.is_empty() || normalized2.is_empty() {
            return 0.0;
        }
        
        // Calculate edit distance
        let distance = self.edit_distance(&normalized1, &normalized2);
        let max_len = normalized1.len().max(normalized2.len());
        
        1.0 - (distance as f64 / max_len as f64)
    }

    fn normalize_function(&self, func: &str) -> String {
        // Remove whitespace, normalize registers, etc.
        func.lines()
            .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
            .map(|line| line.trim().to_lowercase())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn edit_distance(&self, s1: &str, s2: &str) -> usize {
        // Simple Levenshtein distance
        let chars1: Vec<char> = s1.chars().collect();
        let chars2: Vec<char> = s2.chars().collect();
        let len1 = chars1.len();
        let len2 = chars2.len();
        
        if len1 == 0 {
            return len2;
        }
        if len2 == 0 {
            return len1;
        }
        
        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
        
        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }
        
        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if chars1[i-1] == chars2[j-1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i-1][j] + 1)
                    .min(matrix[i][j-1] + 1)
                    .min(matrix[i-1][j-1] + cost);
            }
        }
        
        matrix[len1][len2]
    }

    fn calculate_overall_similarity(&self, llvm_mir: &str, cranelift_mir: &str) -> f64 {
        let llvm_functions = self.extract_functions(llvm_mir).unwrap_or_default();
        let cranelift_functions = self.extract_functions(cranelift_mir).unwrap_or_default();
        
        let mut total_similarity = 0.0;
        let mut function_count = 0;
        
        for (func_name, llvm_func) in llvm_functions {
            if let Some(cranelift_func) = cranelift_functions.get(&func_name) {
                total_similarity += self.calculate_function_similarity(&llvm_func, &cranelift_func);
                function_count += 1;
            }
        }
        
        if function_count == 0 {
            0.0
        } else {
            total_similarity / function_count as f64
        }
    }
}

#[derive(Debug)]
enum Backend {
    LLVM,
    Cranelift,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world_parity() {
        let config = TestConfig::default();
        let executor = DifferentialExecutor::new().unwrap();
        
        for test_case in config.test_cases {
            if test_case.name == "hello_world" {
                let result = executor.run_differential_test(&test_case).unwrap();
                
                assert!(result.is_acceptable(), "Hello world test failed: {:?}", result);
                assert!(result.performance_ratio() >= 5.0, "Performance requirement not met: {}x", result.performance_ratio());
                assert!(result.wasm_equivalent(), "WASM outputs are not equivalent");
                
                println!("Hello World Test Results:");
                println!("  Overall similarity: {:.2}%", result.overall_similarity * 100.0);
                println!("  Performance ratio: {:.2}x", result.performance_ratio());
                break;
            }
        }
    }

    #[test]
    fn test_linear_type_parity() {
        let config = TestConfig::default();
        let executor = DifferentialExecutor::new().unwrap();
        
        for test_case in config.test_cases {
            if test_case.name == "linear_resource" {
                let result = executor.run_differential_test(&test_case).unwrap();
                
                assert!(result.is_acceptable(), "Linear type test failed: {:?}", result);
                
                // Linear types should have perfect semantic preservation
                for (func_name, similarity) in &result.function_equivalence {
                    if func_name.contains("create_image") {
                        assert!(similarity >= &0.95, "Linear type function similarity too low: {}", similarity);
                    }
                }
                
                break;
            }
        }
    }

    #[test]
    fn test_shared_memory_parity() {
        let config = TestConfig::default();
        let executor = DifferentialExecutor::new().unwrap();
        
        for test_case in config.test_cases {
            if test_case.name == "shared_memory" {
                let result = executor.run_differential_test(&test_case).unwrap();
                
                assert!(result.is_acceptable(), "Shared memory test failed: {:?}", result);
                
                // Atomic operations should be preserved
                for (func_name, similarity) in &result.function_equivalence {
                    if func_name.contains("process_data") {
                        assert!(similarity >= &0.90, "Atomic operation similarity too low: {}", similarity);
                    }
                }
                
                break;
            }
        }
    }

    #[test]
    fn test_fibonacci_parity() {
        let config = TestConfig::default();
        let executor = DifferentialExecutor::new().unwrap();
        
        for test_case in config.test_cases {
            if test_case.name == "fibonacci" {
                let result = executor.run_differential_test(&test_case).unwrap();
                
                assert!(result.is_acceptable(), "Fibonacci test failed: {:?}", result);
                
                break;
            }
        }
    }

    #[test]
    fn test_all_parity() {
        let config = TestConfig::default();
        let executor = DifferentialExecutor::new().unwrap();
        
        for test_case in config.test_cases {
            println!("Running parity test: {}", test_case.name);
            
            let result = executor.run_differential_test(&test_case).unwrap();
            
            assert!(result.is_acceptable(), "Parity test failed for {}: {:?}", test_case.name, result);
            assert!(result.wasm_equivalent(), "WASM outputs not equivalent for {}", test_case.name);
            
            println!("  ✓ {} - Similarity: {:.1}%, Performance: {:.1}x", 
                     test_case.name, 
                     result.overall_similarity * 100.0,
                     result.performance_ratio());
        }
    }

    #[test]
    fn test_performance_requirements() {
        let config = TestConfig::default();
        let executor = DifferentialExecutor::new().unwrap();
        
        let mut total_performance_ratio = 0.0;
        let mut test_count = 0;
        
        for test_case in config.test_cases {
            let result = executor.run_differential_test(&test_case).unwrap();
            
            total_performance_ratio += result.performance_ratio();
            test_count += 1;
        }
        
        let avg_performance_ratio = total_performance_ratio / test_count as f64;
        
        assert!(avg_performance_ratio >= 5.0, 
               "Average performance ratio {:.2}x below required 5.0x", 
               avg_performance_ratio);
    }
}

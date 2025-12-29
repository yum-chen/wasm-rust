//! Performance Benchmarks for WasmRust
//! 
//! Comprehensive benchmark suite to validate performance guarantees
//! and ensure regression detection.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::process::Command;
use std::path::Path;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

#[derive(Debug, Clone)]
struct BenchmarkConfig {
    test_cases: Vec<PerformanceTestCase>,
    warmup_iterations: usize,
    measurement_iterations: usize,
}

#[derive(Debug, Clone)]
struct PerformanceTestCase {
    name: String,
    rust_code: String,
    expected_compilation_time_ms: u64,
    expected_binary_size_bytes: usize,
    complexity: ComplexityLevel,
}

#[derive(Debug, Clone, Copy)]
enum ComplexityLevel {
    Simple,      // < 100 lines
    Medium,      // 100-500 lines  
    Complex,      // 500-2000 lines
    VeryComplex,  // > 2000 lines
}

#[derive(Debug, Serialize, Deserialize)]
struct PerformanceResult {
    backend: String,
    test_name: String,
    compilation_time_ms: u64,
    binary_size_bytes: usize,
    memory_usage_bytes: usize,
    optimization_level: String,
    timestamp: std::time::SystemTime,
}

#[derive(Debug, Serialize, Deserialize)]
struct PerformanceComparison {
    test_name: String,
    llvm_result: PerformanceResult,
    cranelift_result: PerformanceResult,
    speedup_ratio: f64,
    size_ratio: f64,
    meets_requirements: bool,
}

impl BenchmarkConfig {
    fn default() -> Self {
        Self {
            test_cases: vec![
                PerformanceTestCase {
                    name: "hello_world".to_string(),
                    rust_code: r#"
#[wasm::export]
pub fn hello() -> String {
    "Hello, World!".to_string()
}
"#.to_string(),
                    expected_compilation_time_ms: 500, // < 500ms for simple case
                    expected_binary_size_bytes: 1024, // < 1KB for simple case
                    complexity: ComplexityLevel::Simple,
                },
                PerformanceTestCase {
                    name: "fibonacci".to_string(),
                    rust_code: r#"
#[wasm::export]
pub fn fibonacci(n: u32) -> u32 {
    match n {
        0 | 1 => n,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

#[wasm::export]
pub fn fibonacci_iterative(n: u32) -> u32 {
    if n <= 1 {
        return n;
    }
    
    let mut a = 0;
    let mut b = 1;
    
    for _ in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }
    
    b
}
"#.to_string(),
                    expected_compilation_time_ms: 1000, // < 1s for medium case
                    expected_binary_size_bytes: 4096, // < 4KB for medium case
                    complexity: ComplexityLevel::Medium,
                },
                PerformanceTestCase {
                    name: "string_processing".to_string(),
                    rust_code: r#"
use std::collections::HashMap;

#[wasm::export]
pub fn word_count(text: &str) -> HashMap<String, u32> {
    let mut counts = HashMap::new();
    
    for word in text.split_whitespace() {
        let word = word.to_lowercase();
        *counts.entry(word).or_insert(0) += 1;
    }
    
    counts
}

#[wasm::export]
pub fn reverse_string(s: &str) -> String {
    s.chars().rev().collect()
}

#[wasm::export]
pub fn is_palindrome(s: &str) -> bool {
    let cleaned: String = s.chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_lowercase())
        .collect();
    
    cleaned.chars().eq(cleaned.chars().rev())
}
"#.to_string(),
                    expected_compilation_time_ms: 2000, // < 2s for complex case
                    expected_binary_size_bytes: 8192, // < 8KB for complex case
                    complexity: ComplexityLevel::Complex,
                },
                PerformanceTestCase {
                    name: "data_structures".to_string(),
                    rust_code: r#"
use wasm::SharedSlice;
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone)]
struct TreeNode {
    value: i32,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
}

impl TreeNode {
    fn new(value: i32) -> Self {
        Self {
            value,
            left: None,
            right: None,
        }
    }
    
    fn insert(&mut self, value: i32) {
        if value < self.value {
            if let Some(ref mut left) = self.left {
                left.insert(value);
            } else {
                self.left = Some(Box::new(TreeNode::new(value)));
            }
        } else if value > self.value {
            if let Some(ref mut right) = self.right {
                right.insert(value);
            } else {
                self.right = Some(Box::new(TreeNode::new(value)));
            }
        }
    }
    
    fn contains(&self, value: i32) -> bool {
        if value == self.value {
            return true;
        }
        
        if value < self.value {
            self.left.as_ref().map_or(false, |left| left.contains(value))
        } else {
            self.right.as_ref().map_or(false, |right| right.contains(value))
        }
    }
}

static NODE_COUNT: AtomicU32 = AtomicU32::new(0);

#[wasm::export]
pub fn create_tree() -> Option<Box<TreeNode>> {
    NODE_COUNT.fetch_add(1, Ordering::SeqCst);
    Some(Box::new(TreeNode::new(42)))
}

#[wasm::export]
pub fn process_tree(node: &TreeNode, target: i32) -> bool {
    node.contains(target)
}

#[wasm::export]
pub fn process_shared_data(data: &SharedSlice<i32>) -> i32 {
    let mut sum = 0;
    for &value in data.iter() {
        sum += value;
    }
    sum
}

#[wasm::export]
pub fn quick_sort(arr: &mut [i32]) {
    if arr.len() <= 1 {
        return;
    }
    
    let pivot = arr[arr.len() / 2];
    let mut left = 0;
    let mut right = arr.len() - 1;
    
    while left <= right {
        while arr[left] < pivot {
            left += 1;
        }
        while arr[right] > pivot {
            right -= 1;
        }
        
        if left <= right {
            arr.swap(left, right);
            left += 1;
            right -= 1;
        }
    }
    
    quick_sort(&mut arr[..left]);
    quick_sort(&mut arr[left + 1..]);
}
"#.to_string(),
                    expected_compilation_time_ms: 3000, // < 3s for very complex case
                    expected_binary_size_bytes: 16384, // < 16KB for very complex case
                    complexity: ComplexityLevel::VeryComplex,
                },
            ],
            warmup_iterations: 3,
            measurement_iterations: 10,
        }
    }
}

struct PerformanceBenchmark {
    temp_dir: TempDir,
}

impl PerformanceBenchmark {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            temp_dir: TempDir::new()?,
        })
    }

    fn benchmark_backend(
        &self,
        test_case: &PerformanceTestCase,
        backend: Backend,
    ) -> Result<PerformanceResult, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        
        // Write test code to temporary file
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
            "--release", // Use release for consistent performance
        ]);
        cmd.current_dir(self.temp_dir.path());
        cmd.env("RUST_LOG", "warn"); // Reduce log noise
        
        let output = cmd.output()?;
        let compilation_time = start_time.elapsed();
        
        if !output.status.success() {
            return Err(format!("Compilation failed: {}", 
                             String::from_utf8_lossy(&output.stderr)).into());
        }
        
        // Get binary size
        let wasm_file = output_dir.join("output.wasm");
        let binary_size = std::fs::metadata(&wasm_file)?.len() as usize;
        
        // Get memory usage (approximate)
        let memory_usage = self.estimate_memory_usage(&wasm_file);
        
        Ok(PerformanceResult {
            backend: match backend {
                Backend::LLVM => "llvm".to_string(),
                Backend::Cranelift => "cranelift".to_string(),
            },
            test_name: test_case.name.clone(),
            compilation_time_ms: compilation_time.as_millis() as u64,
            binary_size_bytes: binary_size,
            memory_usage_bytes: memory_usage,
            optimization_level: "release".to_string(),
            timestamp: std::time::SystemTime::now(),
        })
    }

    fn estimate_memory_usage(&self, wasm_file: &Path) -> usize {
        // This is a simplified estimation
        // In practice, we'd use wasm-tools or runtime measurement
        
        match std::fs::metadata(wasm_file) {
            Ok(metadata) => {
                let file_size = metadata.len();
                // Rough heuristic: WASM runtime memory ≈ 2x file size for typical programs
                file_size * 2
            }
            Err(_) => 0,
        }
    }

    fn run_comparison(
        &self,
        test_case: &PerformanceTestCase,
    ) -> Result<PerformanceComparison, Box<dyn std::error::Error>> {
        // Benchmark both backends
        let llvm_result = self.benchmark_backend(test_case, Backend::LLVM)?;
        let cranelift_result = self.benchmark_backend(test_case, Backend::Cranelift)?;
        
        let speedup_ratio = llvm_result.compilation_time_ms as f64 / 
                            cranelift_result.compilation_time_ms as f64;
        let size_ratio = cranelift_result.binary_size_bytes as f64 / 
                         llvm_result.binary_size_bytes as f64;
        
        let meets_requirements = speedup_ratio >= 5.0 && // 5x speedup
                               cranelift_result.compilation_time_ms <= test_case.expected_compilation_time_ms &&
                               cranelift_result.binary_size_bytes <= test_case.expected_binary_size_bytes;
        
        Ok(PerformanceComparison {
            test_name: test_case.name.clone(),
            llvm_result,
            cranelift_result,
            speedup_ratio,
            size_ratio,
            meets_requirements,
        })
    }
}

#[derive(Debug)]
enum Backend {
    LLVM,
    Cranelift,
}

fn compilation_speed_benchmark(c: &mut Criterion) {
    let config = BenchmarkConfig::default();
    let benchmark = PerformanceBenchmark::new().expect("Failed to create benchmark");
    
    for test_case in config.test_cases {
        let test_name = format!("compilation_speed_{}", test_case.name);
        
        c.bench_function(&test_name, |b| {
            b.iter(|| {
                let result = benchmark.benchmark_backend(&test_case, Backend::Cranelift)
                    .expect("Benchmark failed");
                
                // Verify requirements
                assert!(result.compilation_time_ms <= test_case.expected_compilation_time_ms,
                        "Compilation time {}ms exceeds expected {}ms for {}",
                        result.compilation_time_ms,
                        test_case.expected_compilation_time_ms,
                        test_case.name);
                
                assert!(result.binary_size_bytes <= test_case.expected_binary_size_bytes,
                        "Binary size {} bytes exceeds expected {} bytes for {}",
                        result.binary_size_bytes,
                        test_case.expected_binary_size_bytes,
                        test_case.name);
                
                black_box(result);
            })
        });
    }
}

fn backend_comparison_benchmark(c: &mut Criterion) {
    let config = BenchmarkConfig::default();
    let benchmark = PerformanceBenchmark::new().expect("Failed to create benchmark");
    
    for test_case in config.test_cases {
        let test_name = format!("backend_comparison_{}", test_case.name);
        
        c.bench_function(&test_name, |b| {
            b.iter(|| {
                let comparison = benchmark.run_comparison(&test_case)
                    .expect("Comparison failed");
                
                // Verify 5x speedup requirement
                assert!(comparison.speedup_ratio >= 5.0,
                        "Speedup ratio {:.2}x below required 5.0x for {}",
                        comparison.speedup_ratio,
                        test_case.name);
                
                // Verify binary size is reasonable
                assert!(comparison.size_ratio <= 2.0, // Cranelift shouldn't be > 2x larger
                        "Size ratio {:.2}x exceeds reasonable bound for {}",
                        comparison.size_ratio,
                        test_case.name);
                
                black_box(comparison);
            })
        });
    }
}

fn memory_usage_benchmark(c: &mut Criterion) {
    let config = BenchmarkConfig::default();
    let benchmark = PerformanceBenchmark::new().expect("Failed to create benchmark");
    
    for test_case in config.test_cases {
        let test_name = format!("memory_usage_{}", test_case.name);
        
        c.bench_function(&test_name, |b| {
            b.iter(|| {
                let llvm_result = benchmark.benchmark_backend(&test_case, Backend::LLVM)
                    .expect("LLVM benchmark failed");
                let cranelift_result = benchmark.benchmark_backend(&test_case, Backend::Cranelift)
                    .expect("Cranelift benchmark failed");
                
                // Memory usage should be comparable
                let memory_ratio = cranelift_result.memory_usage_bytes as f64 / 
                                  llvm_result.memory_usage_bytes as f64;
                
                assert!(memory_ratio >= 0.5 && memory_ratio <= 2.0,
                        "Memory ratio {:.2}x outside reasonable bounds for {}",
                        memory_ratio,
                        test_case.name);
                
                black_box((llvm_result, cranelift_result));
            })
        });
    }
}

fn regression_detection_benchmark(c: &mut Criterion) {
    let config = BenchmarkConfig::default();
    let benchmark = PerformanceBenchmark::new().expect("Failed to create benchmark");
    
    // This test establishes performance baselines
    let mut baselines = HashMap::new();
    
    for test_case in config.test_cases {
        let comparison = benchmark.run_comparison(&test_case)
            .expect("Baseline establishment failed");
        
        baselines.insert(test_case.name.clone(), comparison);
        
        // Store baseline results (in practice, would save to file)
        println!("Baseline for {}: {:.2}x speedup, {:.2}x size ratio",
                 test_case.name,
                 comparison.speedup_ratio,
                 comparison.size_ratio);
    }
    
    // In CI, we would compare against stored baselines
    // For now, just establish that we meet requirements
    for (_, baseline) in baselines {
        assert!(baseline.meets_requirements,
                "Performance baseline does not meet requirements for {}",
                baseline.test_name);
    }
}

criterion_group!(
    benches,
    compilation_speed_benchmark,
    backend_comparison_benchmark,
    memory_usage_benchmark,
    regression_detection_benchmark
);

criterion_main!(benches);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hello_world_performance() {
        let config = BenchmarkConfig::default();
        let benchmark = PerformanceBenchmark::new().unwrap();
        
        let test_case = &config.test_cases[0]; // hello_world
        let comparison = benchmark.run_comparison(test_case).unwrap();
        
        assert!(comparison.speedup_ratio >= 5.0, 
                "Hello world speedup {:.2}x below 5.0x requirement", 
                comparison.speedup_ratio);
        assert!(comparison.meets_requirements, 
                "Hello world doesn't meet performance requirements");
    }

    #[test]
    fn test_binary_size_requirements() {
        let config = BenchmarkConfig::default();
        let benchmark = PerformanceBenchmark::new().unwrap();
        
        for test_case in config.test_cases {
            let result = benchmark.benchmark_backend(&test_case, Backend::Cranelift).unwrap();
            
            assert!(result.binary_size_bytes <= test_case.expected_binary_size_bytes,
                    "Binary size {} exceeds expected {} for {}",
                    result.binary_size_bytes,
                    test_case.expected_binary_size_bytes,
                    test_case.name);
        }
    }

    #[test]
    fn test_compilation_time_requirements() {
        let config = BenchmarkConfig::default();
        let benchmark = PerformanceBenchmark::new().unwrap();
        
        for test_case in config.test_cases {
            let result = benchmark.benchmark_backend(&test_case, Backend::Cranelift).unwrap();
            
            assert!(result.compilation_time_ms <= test_case.expected_compilation_time_ms,
                    "Compilation time {}ms exceeds expected {}ms for {}",
                    result.compilation_time_ms,
                    test_case.expected_compilation_time_ms,
                    test_case.name);
        }
    }

    #[test]
    fn test_performance_regression_detection() {
        // This would normally load historical baselines and compare
        // For now, just verify we can establish baselines
        
        let config = BenchmarkConfig::default();
        let benchmark = PerformanceBenchmark::new().unwrap();
        
        let mut all_pass = true;
        
        for test_case in config.test_cases {
            let comparison = benchmark.run_comparison(&test_case).unwrap();
            
            if !comparison.meets_requirements {
                println!("Test {} doesn't meet requirements: speedup={:.2}x, size_ratio={:.2}x",
                         test_case.name,
                         comparison.speedup_ratio,
                         comparison.size_ratio);
                all_pass = false;
            }
        }
        
        assert!(all_pass, "Some performance tests don't meet requirements");
    }
}

//! Schema definitions for WasmRust Curation Registry
//!
//! This module defines the core data structures used throughout the registry.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Main crate metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrateMetadata {
    /// Unique identifier for the crate
    pub id: String,
    /// Crate name
    pub name: String,
    /// Crate version (semver)
    pub version: String,
    /// Crate description
    pub description: String,
    /// Authors of the crate
    pub authors: Vec<String>,
    /// License information
    pub license: String,
    /// Repository URL
    pub repository: String,
    /// Documentation URL
    pub documentation: Option<String>,
    /// Keywords for search
    pub keywords: Vec<String>,
    /// Categories (WASM-specific categories)
    pub categories: Vec<String>,
    /// WASM compatibility status
    pub wasm_compatibility: WasmCompatibility,
    /// GC readiness status
    pub gc_ready: bool,
    /// Dual compilation support
    pub dual_compilation: bool,
    /// Dependencies with version requirements
    pub dependencies: HashMap<String, String>,
    /// Development dependencies
    pub dev_dependencies: HashMap<String, String>,
    /// Build dependencies
    pub build_dependencies: HashMap<String, String>,
    /// Test results from automated testing
    pub test_results: Vec<TestResult>,
    /// Crate size in bytes
    pub crate_size: u64,
    /// WASM binary size in bytes
    pub wasm_size: Option<u64>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
    /// Crate status in the registry
    pub status: CrateStatus,
    /// Digital signatures for verification
    pub signatures: Vec<Signature>,
    /// Fork information (if this is a forked crate)
    pub fork_info: Option<ForkInfo>,
}

/// WASM compatibility status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmCompatibility {
    /// Overall compatibility level
    pub level: CompatibilityLevel,
    /// Compilation status
    pub compilation: CompilationStatus,
    /// Runtime behavior status
    pub runtime: RuntimeStatus,
    /// Performance characteristics
    pub performance: PerformanceStatus,
    /// Compatibility notes
    pub notes: Vec<String>,
}

/// Compatibility levels for WASM support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompatibilityLevel {
    /// Fully compatible - ready for production use
    ProductionReady,
    /// Mostly compatible - minor issues
    Compatible,
    /// Partially compatible - significant limitations
    Partial,
    /// Not compatible - requires major changes
    Incompatible,
    /// Not tested
    Unknown,
}

/// Compilation status details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationStatus {
    /// Can compile to WASM
    pub compiles: bool,
    /// Compilation warnings
    pub warnings: Vec<String>,
    /// Compilation errors (if any)
    pub errors: Vec<String>,
    /// Compilation time in milliseconds
    pub compilation_time_ms: Option<u64>,
    /// Memory usage during compilation
    pub memory_usage_mb: Option<u64>,
}

/// Runtime status details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeStatus {
    /// Can execute in WASM environment
    pub executes: bool,
    /// Runtime errors (if any)
    pub errors: Vec<String>,
    /// Memory usage during execution
    pub memory_usage_mb: Option<u64>,
    /// Execution time variance compared to native
    pub performance_variance: Option<f64>,
}

/// Performance status details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceStatus {
    /// Performance compared to native (0.0-1.0, where 1.0 is equal)
    pub native_comparison: f64,
    /// WASM binary size compared to Rust binary size
    pub size_comparison: f64,
    /// Performance benchmarks
    pub benchmarks: HashMap<String, BenchmarkResult>,
}

/// Benchmark result for specific operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Operation name/description
    pub operation: String,
    /// Native execution time (nanoseconds)
    pub native_time_ns: u64,
    /// WASM execution time (nanoseconds)
    pub wasm_time_ns: u64,
    /// Memory usage difference
    pub memory_delta_mb: i64,
    /// Performance ratio (WASM/native)
    pub performance_ratio: f64,
}

/// Test result from automated testing pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    /// Test type identifier
    pub test_type: TestType,
    /// Timestamp when test was run
    pub timestamp: DateTime<Utc>,
    /// Test outcome
    pub outcome: TestOutcome,
    /// Detailed test results
    pub details: HashMap<String, serde_json::Value>,
    /// Test duration in milliseconds
    pub duration_ms: u64,
    /// Test environment details
    pub environment: TestEnvironment,
}

/// Types of automated tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestType {
    /// Basic compilation test
    Compilation,
    /// Unit tests execution
    UnitTests,
    /// Integration tests
    IntegrationTests,
    /// Performance benchmarks
    Performance,
    /// WASM-specific tests
    WasmSpecific,
    /// GC compatibility tests
    GcCompatibility,
    /// Memory usage tests
    MemoryUsage,
    /// Cross-platform compatibility
    CrossPlatform,
}

/// Test outcome
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestOutcome {
    /// All tests passed
    Passed,
    /// Some tests failed
    Failed,
    /// Tests timed out
    Timeout,
    /// Tests could not be run
    Skipped,
    /// Test execution error
    Error,
}

/// Test environment details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestEnvironment {
    /// Rust toolchain version
    pub rust_version: String,
    /// WASM target
    pub wasm_target: String,
    /// WASM runtime version
    pub wasm_runtime: String,
    /// Operating system
    pub os: String,
    /// Architecture
    pub arch: String,
    /// Additional environment variables
    pub environment_vars: HashMap<String, String>,
}

/// Crate status in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrateStatus {
    /// Newly submitted, waiting for testing
    Pending,
    /// Currently undergoing testing
    Testing,
    /// Under manual review
    UnderReview,
    /// Approved and available
    Approved,
    /// Rejected (with reason)
    Rejected(String),
    /// Deprecated (superseded by newer version)
    Deprecated,
    /// Removed from registry
    Removed,
}

/// Digital signature for crate verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    /// Signer identifier (e.g., GPG key ID)
    pub signer: String,
    /// Signature data
    pub signature: String,
    /// Signing timestamp
    pub timestamp: DateTime<Utc>,
    /// Signature algorithm
    pub algorithm: String,
}

/// Fork information for forked crates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkInfo {
    /// Original crate name
    pub original_name: String,
    /// Original crate version
    pub original_version: String,
    /// Fork reason
    pub reason: ForkReason,
    /// Applied patches
    pub patches: Vec<Patch>,
    /// Sync status with upstream
    pub sync_status: SyncStatus,
    /// Last sync attempt timestamp
    pub last_sync: Option<DateTime<Utc>>,
}

/// Reasons for forking a crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForkReason {
    /// WASM compatibility patches
    WasmCompatibility,
    /// Performance optimizations
    Performance,
    /// Bug fixes not accepted upstream
    BugFixes,
    /// Feature additions
    Features,
    /// Security fixes
    Security,
}

/// Patch applied to forked crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Patch name/identifier
    pub name: String,
    /// Patch description
    pub description: String,
    /// Patch author
    pub author: String,
    /// Applied date
    pub applied_date: DateTime<Utc>,
    /// Patch content (diff)
    pub diff: String,
}

/// Sync status with upstream crate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncStatus {
    /// In sync with upstream
    Synced,
    /// Out of sync, patches available
    OutOfSync,
    /// Upstream unavailable
    UpstreamUnavailable,
    /// Manual sync required
    ManualSyncRequired,
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registry name/identifier
    pub name: String,
    /// Registry URL
    pub url: String,
    /// Contact email
    pub contact_email: String,
    /// Public key for verification
    pub public_key: String,
    /// Supported WASM targets
    pub supported_targets: Vec<String>,
    /// Required test types
    pub required_tests: Vec<TestType>,
    /// Quality gates
    pub quality_gates: QualityGates,
    /// Sync configuration
    pub sync_config: SyncConfig,
}

/// Quality gates for crate approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityGates {
    /// Minimum WASM compatibility level required
    pub min_compatibility: CompatibilityLevel,
    /// Required test pass rate (0.0-1.0)
    pub test_pass_rate: f64,
    /// Maximum WASM size increase factor
    pub max_size_increase: f64,
    /// Maximum performance degradation
    pub max_performance_loss: f64,
    /// Required documentation coverage
    pub documentation_coverage: f64,
}

/// Synchronization configuration for registry federation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    /// Other registry instances to sync with
    pub peer_registries: Vec<String>,
    /// Sync interval in seconds
    pub sync_interval: u64,
    /// Maximum sync retries
    pub max_retries: u32,
    /// Conflict resolution strategy
    pub conflict_resolution: ConflictResolution,
}

/// Conflict resolution strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Prefer local changes
    PreferLocal,
    /// Prefer remote changes
    PreferRemote,
    /// Manual resolution required
    Manual,
    /// Use timestamp (newer wins)
    Timestamp,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// Response status
    pub status: ResponseStatus,
    /// Response data
    pub data: Option<T>,
    /// Error message (if any)
    pub error: Option<String>,
    /// Pagination info
    pub pagination: Option<Pagination>,
}

/// Response status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseStatus {
    /// Request succeeded
    Success,
    /// Request failed
    Error,
    /// Partial success
    Partial,
}

/// Pagination information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    /// Current page
    pub page: u32,
    /// Page size
    pub page_size: u32,
    /// Total number of items
    pub total: u64,
    /// Total number of pages
    pub total_pages: u32,
}
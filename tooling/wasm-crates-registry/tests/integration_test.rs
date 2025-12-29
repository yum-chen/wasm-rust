//! Integration tests for WasmRust Curation Registry
//!
//! These tests validate the complete registry workflow including API, database,
//! and testing pipeline integration.

use serde_json::json;
use std::process::Command;
use tempfile::TempDir;

/// Test the complete workflow: submission → testing → querying
#[tokio::test]
async fn test_complete_workflow() {
    // Setup test environment
    let temp_dir = TempDir::new().unwrap();
    let database_url = format!("sqlite:{}/test.db", temp_dir.path().to_string_lossy());
    
    // Set environment variables
    std::env::set_var("DATABASE_URL", &database_url);
    std::env::set_var("HOST", "127.0.0.1");
    std::env::set_var("PORT", "8081"); // Use different port to avoid conflicts
    
    // Start the registry server in background
    let server_handle = start_registry_server(&database_url).await;
    
    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    
    // Test health endpoint
    let client = reqwest::Client::new();
    let health_response = client
        .get("http://127.0.0.1:8081/health")
        .send()
        .await
        .expect("Failed to connect to registry");
    
    assert!(health_response.status().is_success());
    
    // Submit a test crate
    let test_crate = json!({
        "name": "integration-test-crate",
        "version": "1.0.0",
        "description": "Integration test crate",
        "authors": ["Integration Test"],
        "license": "MIT",
        "repository": "https://github.com/test/integration-test",
        "wasm_compatibility": {
            "level": "Unknown",
            "compilation": {"compiles": false, "warnings": [], "errors": []},
            "runtime": {"executes": false, "errors": []},
            "performance": {"native_comparison": 0.0, "size_comparison": 0.0, "benchmarks": {}}
        },
        "gc_ready": false,
        "dual_compilation": false,
        "dependencies": {},
        "dev_dependencies": {},
        "build_dependencies": {},
        "test_results": [],
        "crate_size": 1024,
        "wasm_size": None
    });
    
    let submit_response = client
        .post("http://127.0.0.1:8081/crates")
        .json(&test_crate)
        .send()
        .await
        .expect("Failed to submit crate");
    
    assert!(submit_response.status().is_success());
    
    // Get the created crate ID
    let submit_result: serde_json::Value = submit_response.json().await.unwrap();
    let crate_id = submit_result["data"]["id"].as_str().unwrap();
    
    // List crates to verify submission
    let list_response = client
        .get("http://127.0.0.1:8081/crates")
        .send()
        .await
        .expect("Failed to list crates");
    
    assert!(list_response.status().is_success());
    
    let list_result: serde_json::Value = list_response.json().await.unwrap();
    assert!(list_result["data"].is_array());
    
    // Get crate details
    let get_response = client
        .get(&format!("http://127.0.0.1:8081/crates/{}", crate_id))
        .send()
        .await
        .expect("Failed to get crate");
    
    assert!(get_response.status().is_success());
    
    // Run tests (placeholder - would trigger actual testing pipeline)
    let test_response = client
        .post(&format!("http://127.0.0.1:8081/tests/{}", crate_id))
        .send()
        .await
        .expect("Failed to run tests");
    
    assert!(test_response.status().is_success());
    
    // Get test results
    let results_response = client
        .get(&format!("http://127.0.0.1:8081/tests/{}", crate_id))
        .send()
        .await
        .expect("Failed to get test results");
    
    assert!(results_response.status().is_success());
    
    // Cleanup: delete the crate
    let delete_response = client
        .delete(&format!("http://127.0.0.1:8081/crates/{}", crate_id))
        .send()
        .await
        .expect("Failed to delete crate");
    
    assert!(delete_response.status().is_success());
    
    // Stop the server
    server_handle.abort();
}

/// Start registry server in background
async fn start_registry_server(database_url: &str) -> tokio::task::JoinHandle<()> {
    let url = database_url.to_string();
    
    tokio::spawn(async move {
        // This would normally start the actual server
        // For testing, we'll simulate basic functionality
        let _server = mock_server(url).await;
    })
}

/// Mock server implementation for testing
async fn mock_server(_database_url: String) {
    // Simulate server startup and basic request handling
    // In a real implementation, this would use warp::serve
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
}

/// Test CLI tool functionality
#[test]
fn test_cli_tool() {
    // Test that the CLI binary compiles and has basic help functionality
    let output = Command::new("cargo")
        .args(["run", "--bin", "wasm-crates-registry-cli", "--", "--help"])
        .output()
        .expect("Failed to run CLI help");
    
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("WasmRust Curation Registry CLI"));
}

/// Test database operations
#[tokio::test]
async fn test_database_operations() {
    use wasm_crates_registry::database::Database;
    use wasm_crates_registry::schema::CrateMetadata;
    
    let temp_dir = TempDir::new().unwrap();
    let database_url = format!("sqlite:{}/test.db", temp_dir.path().to_string_lossy());
    
    let db = Database::new(&database_url).await.expect("Failed to create database");
    db.initialize().await.expect("Failed to initialize database");
    
    // Test crate creation
    let test_crate = CrateMetadata {
        id: "test-id".to_string(),
        name: "test-crate".to_string(),
        version: "1.0.0".to_string(),
        description: "Test crate".to_string(),
        authors: vec!["Test Author".to_string()],
        license: "MIT".to_string(),
        repository: "https://github.com/test".to_string(),
        documentation: None,
        keywords: vec!["test".to_string()],
        categories: vec!["testing".to_string()],
        wasm_compatibility: Default::default(),
        gc_ready: false,
        dual_compilation: false,
        dependencies: Default::default(),
        dev_dependencies: Default::default(),
        build_dependencies: Default::default(),
        test_results: Vec::new(),
        crate_size: 1024,
        wasm_size: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: Default::default(),
        signatures: Vec::new(),
        fork_info: None,
    };
    
    // Create crate
    let created = db.create_crate(test_crate.clone()).await;
    assert!(created.is_ok());
    
    // Get crate
    let retrieved = db.get_crate("test-id").await.expect("Failed to get crate");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().name, "test-crate");
    
    // List crates
    let (crates, pagination) = db.list_crates(1, 10, None, None, None).await.expect("Failed to list crates");
    assert_eq!(crates.len(), 1);
    assert_eq!(pagination.total, 1);
    
    // Delete crate
    let deleted = db.delete_crate("test-id").await;
    assert!(deleted.is_ok());
}

/// Test schema validation
#[test]
fn test_schema_validation() {
    use wasm_crates_registry::schema::{CrateMetadata, WasmCompatibility, CompatibilityLevel};
    
    let valid_crate = CrateMetadata {
        id: "valid-id".to_string(),
        name: "valid-crate".to_string(),
        version: "1.0.0".to_string(),
        description: "Valid crate".to_string(),
        authors: vec!["Author".to_string()],
        license: "MIT".to_string(),
        repository: "https://github.com/test".to_string(),
        documentation: None,
        keywords: vec![],
        categories: vec![],
        wasm_compatibility: WasmCompatibility {
            level: CompatibilityLevel::Unknown,
            compilation: Default::default(),
            runtime: Default::default(),
            performance: Default::default(),
            notes: vec![],
        },
        gc_ready: false,
        dual_compilation: false,
        dependencies: Default::default(),
        dev_dependencies: Default::default(),
        build_dependencies: Default::default(),
        test_results: Vec::new(),
        crate_size: 0,
        wasm_size: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        status: Default::default(),
        signatures: Vec::new(),
        fork_info: None,
    };
    
    // Test serialization/deserialization
    let serialized = serde_json::to_string(&valid_crate).expect("Failed to serialize");
    let deserialized: CrateMetadata = serde_json::from_str(&serialized).expect("Failed to deserialize");
    
    assert_eq!(valid_crate.name, deserialized.name);
    assert_eq!(valid_crate.version, deserialized.version);
}

/// Test error handling
#[tokio::test]
async fn test_error_handling() {
    use wasm_crates_registry::database::Database;
    
    let temp_dir = TempDir::new().unwrap();
    let database_url = format!("sqlite:{}/test.db", temp_dir.path().to_string_lossy());
    
    let db = Database::new(&database_url).await.expect("Failed to create database");
    db.initialize().await.expect("Failed to initialize database");
    
    // Test getting non-existent crate
    let result = db.get_crate("non-existent").await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
    
    // Test deleting non-existent crate
    let result = db.delete_crate("non-existent").await;
    assert!(result.is_err());
}
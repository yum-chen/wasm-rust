//! WasmRust Curation Registry CLI Tool
//!
//! Command-line interface for interacting with the WasmRust Curation Registry.

use clap::{Parser, Subcommand};
use reqwest::Client;
use serde_json::json;
use std::process;
use std::time::Duration;

/// CLI arguments structure
#[derive(Parser)]
#[command(name = "wasm-crates")]
#[command(about = "WasmRust Curation Registry CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    
    /// Registry server URL
    #[arg(long, default_value = "http://localhost:8080")]
    registry_url: String,
    
    /// API key for authentication
    #[arg(long)]
    api_key: Option<String>,
    
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Available commands
#[derive(Subcommand)]
enum Commands {
    /// Submit a crate to the registry
    Submit {
        /// Path to Cargo.toml
        #[arg(short, long)]
        manifest: String,
        
        /// Force submission even if validation fails
        #[arg(short, long)]
        force: bool,
    },
    
    /// List crates in the registry
    List {
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        
        /// Filter by WASM compatibility
        #[arg(long)]
        compatibility: Option<String>,
        
        /// Search term
        #[arg(short, long)]
        search: Option<String>,
        
        /// Page number
        #[arg(long, default_value = "1")]
        page: u32,
        
        /// Page size
        #[arg(long, default_value = "20")]
        page_size: u32,
    },
    
    /// Get crate details
    Get {
        /// Crate ID or name
        crate_id: String,
    },
    
    /// Run tests for a crate
    Test {
        /// Crate ID
        crate_id: String,
    },
    
    /// Get test results
    Results {
        /// Crate ID
        crate_id: String,
    },
    
    /// Sync with remote registries
    Sync,
    
    /// Check registry health
    Health,
    
    /// Get registry configuration
    Config,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    // Create HTTP client
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    
    match cli.command {
        Commands::Health => {
            check_health(&client, &cli.registry_url).await?;
        }
        Commands::List { status, compatibility, search, page, page_size } => {
            list_crates(&client, &cli.registry_url, status, compatibility, search, page, page_size).await?;
        }
        Commands::Get { crate_id } => {
            get_crate(&client, &cli.registry_url, &crate_id).await?;
        }
        Commands::Test { crate_id } => {
            run_tests(&client, &cli.registry_url, &crate_id).await?;
        }
        Commands::Results { crate_id } => {
            get_test_results(&client, &cli.registry_url, &crate_id).await?;
        }
        Commands::Submit { manifest, force } => {
            submit_crate(&client, &cli.registry_url, &manifest, force).await?;
        }
        Commands::Sync => {
            sync_registry(&client, &cli.registry_url).await?;
        }
        Commands::Config => {
            get_config(&client, &cli.registry_url).await?;
        }
    }
    
    Ok(())
}

/// Check registry health
async fn check_health(client: &Client, registry_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/health", registry_url))
        .send()
        .await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        println!("✅ Registry is healthy");
        if let Some(data) = api_response.get("data") {
            println!("Message: {}", data);
        }
    } else {
        println!("❌ Registry is not healthy");
        eprintln!("Status: {}", response.status());
    }
    
    Ok(())
}

/// List crates in the registry
async fn list_crates(
    client: &Client, 
    registry_url: &str, 
    status: Option<String>,
    compatibility: Option<String>,
    search: Option<String>,
    page: u32,
    page_size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut url = format!("{}/crates?page={}&page_size={}", registry_url, page, page_size);
    
    if let Some(status) = status {
        url.push_str(&format!("&status={}", status));
    }
    
    if let Some(compatibility) = compatibility {
        url.push_str(&format!("&compatibility={}", compatibility));
    }
    
    if let Some(search) = search {
        url.push_str(&format!("&search={}", search));
    }
    
    let response = client.get(&url).send().await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        
        if let Some(data) = api_response.get("data") {
            if let Some(crates) = data.as_array() {
                println!("Found {} crates:", crates.len());
                println!();
                
                for crate_metadata in crates {
                    if let (Some(name), Some(version), Some(status), Some(compatibility)) = (
                        crate_metadata.get("name"),
                        crate_metadata.get("version"),
                        crate_metadata.get("status"),
                        crate_metadata.get("wasm_compatibility"),
                    ) {
                        let comp_level = compatibility.get("level")
                            .and_then(|l| l.as_str())
                            .unwrap_or("Unknown");
                            
                        println!("📦 {} v{}", name, version);
                        println!("   Status: {}", status);
                        println!("   WASM Compatibility: {}", comp_level);
                        
                        if let Some(description) = crate_metadata.get("description") {
                            if let Some(desc) = description.as_str() {
                                if !desc.is_empty() {
                                    println!("   Description: {}", desc);
                                }
                            }
                        }
                        
                        println!();
                    }
                }
                
                if let Some(pagination) = api_response.get("pagination") {
                    if let (Some(page), Some(total_pages), Some(total)) = (
                        pagination.get("page"),
                        pagination.get("total_pages"),
                        pagination.get("total"),
                    ) {
                        println!("Page {}/{} (Total: {})", page, total_pages, total);
                    }
                }
            }
        }
    } else {
        eprintln!("❌ Failed to list crates: {}", response.status());
    }
    
    Ok(())
}

/// Get crate details
async fn get_crate(client: &Client, registry_url: &str, crate_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/crates/{}", registry_url, crate_id))
        .send()
        .await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        
        if let Some(data) = api_response.get("data") {
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    } else {
        eprintln!("❌ Failed to get crate: {}", response.status());
        if let Ok(error_body) = response.text().await {
            eprintln!("Error: {}", error_body);
        }
    }
    
    Ok(())
}

/// Run tests for a crate
async fn run_tests(client: &Client, registry_url: &str, crate_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/tests/{}", registry_url, crate_id))
        .send()
        .await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        
        if let Some(data) = api_response.get("data") {
            println!("✅ Tests started successfully");
            println!("Message: {}", data);
        }
    } else {
        eprintln!("❌ Failed to run tests: {}", response.status());
    }
    
    Ok(())
}

/// Get test results for a crate
async fn get_test_results(client: &Client, registry_url: &str, crate_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/tests/{}", registry_url, crate_id))
        .send()
        .await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        
        if let Some(data) = api_response.get("data") {
            println!("📊 Test Results for crate {}", crate_id);
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    } else {
        eprintln!("❌ Failed to get test results: {}", response.status());
    }
    
    Ok(())
}

/// Submit a crate to the registry
async fn submit_crate(client: &Client, registry_url: &str, manifest_path: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    // In a real implementation, this would:
    // 1. Parse Cargo.toml
    // 2. Extract crate metadata
    // 3. Create CrateMetadata structure
    // 4. Submit to registry
    
    // For now, create placeholder metadata
    let crate_metadata = json!({
        "name": "placeholder-crate",
        "version": "0.1.0",
        "description": "Placeholder crate for demonstration",
        "authors": ["WasmRust Team"],
        "license": "MIT OR Apache-2.0",
        "repository": "https://github.com/wasmrust/placeholder",
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
        "crate_size": 0,
        "wasm_size": None
    });
    
    let response = client
        .post(&format!("{}/crates", registry_url))
        .json(&crate_metadata)
        .send()
        .await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        
        if let Some(data) = api_response.get("data") {
            println!("✅ Crate submitted successfully");
            if let Some(id) = data.get("id") {
                println!("Crate ID: {}", id);
            }
        }
    } else {
        eprintln!("❌ Failed to submit crate: {}", response.status());
        if let Ok(error_body) = response.text().await {
            eprintln!("Error: {}", error_body);
        }
    }
    
    Ok(())
}

/// Sync with remote registries
async fn sync_registry(client: &Client, registry_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .post(&format!("{}/registry/sync", registry_url))
        .send()
        .await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        
        if let Some(data) = api_response.get("data") {
            println!("✅ Registry sync completed");
            println!("Message: {}", data);
        }
    } else {
        eprintln!("❌ Failed to sync registry: {}", response.status());
    }
    
    Ok(())
}

/// Get registry configuration
async fn get_config(client: &Client, registry_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let response = client
        .get(&format!("{}/registry", registry_url))
        .send()
        await?;
    
    if response.status().is_success() {
        let api_response: serde_json::Value = response.json().await?;
        
        if let Some(data) = api_response.get("data") {
            println!("🔧 Registry Configuration");
            println!("{}", serde_json::to_string_pretty(data)?);
        }
    } else {
        eprintln!("❌ Failed to get configuration: {}", response.status());
    }
    
    Ok(())
}
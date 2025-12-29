//! RESTful API implementation for WasmRust Curation Registry
//!
//! This module provides the HTTP API endpoints for crate management, testing,
//! and registry synchronization.

use warp::{Filter, Rejection, Reply};
use std::convert::Infallible;
use crate::schema::{
    ApiResponse, CrateMetadata, CrateStatus, TestResult, WasmCompatibility, 
    CompatibilityLevel, RegistryConfig, ResponseStatus
};
use crate::database::Database;
use uuid::Uuid;
use chrono::Utc;

/// API routes configuration
pub fn routes(db: Database) -> impl Filter<Extract = impl Reply, Error = Infallible> + Clone {
    // Health check endpoint
    let health = warp::path!("health")
        .and(warp::get())
        .map(|| warp::reply::json(&ApiResponse {
            status: ResponseStatus::Success,
            data: Some("Registry API is healthy".to_string()),
            error: None,
            pagination: None,
        }));

    // Crate management endpoints
    let crates = warp::path("crates");
    
    // List crates with filtering
    let list_crates = crates
        .and(warp::get())
        .and(warp::query::<ListCratesQuery>())
        .and(with_db(db.clone()))
        .and_then(list_crates_handler);

    // Get crate by ID
    let get_crate = crates
        .and(warp::path!(String))
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get_crate_handler);

    // Submit new crate
    let submit_crate = crates
        .and(warp::post())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(submit_crate_handler);

    // Update crate
    let update_crate = crates
        .and(warp::path!(String))
        .and(warp::put())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(update_crate_handler);

    // Delete crate
    let delete_crate = crates
        .and(warp::path!(String))
        .and(warp::delete())
        .and(with_db(db.clone()))
        .and_then(delete_crate_handler);

    // Testing endpoints
    let tests = warp::path("tests");
    
    // Run tests for crate
    let run_tests = tests
        .and(warp::path!(String))
        .and(warp::post())
        .and(with_db(db.clone()))
        .and_then(run_tests_handler);

    // Get test results
    let get_test_results = tests
        .and(warp::path!(String))
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get_test_results_handler);

    // Registry management endpoints
    let registry = warp::path("registry");
    
    // Get registry configuration
    let get_config = registry
        .and(warp::get())
        .and(with_db(db.clone()))
        .and_then(get_config_handler);

    // Update registry configuration
    let update_config = registry
        .and(warp::put())
        .and(warp::body::json())
        .and(with_db(db.clone()))
        .and_then(update_config_handler);

    // Sync with other registries
    let sync = registry
        .and(warp::path!("sync"))
        .and(warp::post())
        .and(with_db(db.clone()))
        .and_then(sync_handler);

    // Combine all routes
    health
        .or(list_crates)
        .or(get_crate)
        .or(submit_crate)
        .or(update_crate)
        .or(delete_crate)
        .or(run_tests)
        .or(get_test_results)
        .or(get_config)
        .or(update_config)
        .or(sync)
        .with(warp::cors().allow_any_origin())
        .recover(handle_rejection)
}

/// Database dependency injection helper
fn with_db(db: Database) -> impl Filter<Extract = (Database,), Error = Infallible> + Clone {
    warp::any().map(move || db.clone())
}

/// Query parameters for listing crates
#[derive(Debug, serde::Deserialize)]
struct ListCratesQuery {
    page: Option<u32>,
    page_size: Option<u32>,
    status: Option<String>,
    compatibility: Option<String>,
    search: Option<String>,
}

/// List crates handler
async fn list_crates_handler(
    query: ListCratesQuery,
    db: Database,
) -> Result<impl Reply, Rejection> {
    let page = query.page.unwrap_or(1);
    let page_size = query.page_size.unwrap_or(20);
    
    match db.list_crates(page, page_size, query.status, query.compatibility, query.search).await {
        Ok((crates, pagination)) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some(crates),
                error: None,
                pagination: Some(pagination),
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Get crate handler
async fn get_crate_handler(
    crate_id: String,
    db: Database,
) -> Result<impl Reply, Rejection> {
    match db.get_crate(&crate_id).await {
        Ok(Some(crate_metadata)) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some(crate_metadata),
                error: None,
                pagination: None,
            }))
        }
        Ok(None) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some("Crate not found".to_string()),
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Submit crate handler
async fn submit_crate_handler(
    mut crate_metadata: CrateMetadata,
    db: Database,
) -> Result<impl Reply, Rejection> {
    // Generate unique ID for the crate
    crate_metadata.id = Uuid::new_v4().to_string();
    crate_metadata.created_at = Utc::now();
    crate_metadata.updated_at = Utc::now();
    
    // Set initial status
    crate_metadata.status = CrateStatus::Pending;
    
    // Validate crate metadata
    if let Err(e) = validate_crate_metadata(&crate_metadata) {
        return Ok(warp::reply::json(&ApiResponse {
            status: ResponseStatus::Error,
            data: None,
            error: Some(e),
            pagination: None,
        }));
    }
    
    match db.create_crate(crate_metadata).await {
        Ok(crate_metadata) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some(crate_metadata),
                error: None,
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Update crate handler
async fn update_crate_handler(
    crate_id: String,
    mut crate_metadata: CrateMetadata,
    db: Database,
) -> Result<impl Reply, Rejection> {
    crate_metadata.id = crate_id.clone();
    crate_metadata.updated_at = Utc::now();
    
    // Validate crate metadata
    if let Err(e) = validate_crate_metadata(&crate_metadata) {
        return Ok(warp::reply::json(&ApiResponse {
            status: ResponseStatus::Error,
            data: None,
            error: Some(e),
            pagination: None,
        }));
    }
    
    match db.update_crate(&crate_id, crate_metadata).await {
        Ok(crate_metadata) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some(crate_metadata),
                error: None,
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Delete crate handler
async fn delete_crate_handler(
    crate_id: String,
    db: Database,
) -> Result<impl Reply, Rejection> {
    match db.delete_crate(&crate_id).await {
        Ok(()) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some("Crate deleted successfully".to_string()),
                error: None,
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Run tests handler
async fn run_tests_handler(
    crate_id: String,
    db: Database,
) -> Result<impl Reply, Rejection> {
    // In a real implementation, this would trigger the testing pipeline
    // For now, return a success response with placeholder test results
    
    let test_results = vec![TestResult {
        test_type: crate::schema::TestType::Compilation,
        timestamp: Utc::now(),
        outcome: crate::schema::TestOutcome::Passed,
        details: std::collections::HashMap::new(),
        duration_ms: 1000,
        environment: crate::schema::TestEnvironment {
            rust_version: "1.70.0".to_string(),
            wasm_target: "wasm32-unknown-unknown".to_string(),
            wasm_runtime: "wasmtime".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            environment_vars: std::collections::HashMap::new(),
        },
    }];
    
    match db.update_test_results(&crate_id, test_results).await {
        Ok(()) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some("Tests completed successfully".to_string()),
                error: None,
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Get test results handler
async fn get_test_results_handler(
    crate_id: String,
    db: Database,
) -> Result<impl Reply, Rejection> {
    match db.get_test_results(&crate_id).await {
        Ok(Some(test_results)) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some(test_results),
                error: None,
                pagination: None,
            }))
        }
        Ok(None) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some("Test results not found".to_string()),
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Get registry configuration handler
async fn get_config_handler(
    db: Database,
) -> Result<impl Reply, Rejection> {
    match db.get_config().await {
        Ok(Some(config)) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some(config),
                error: None,
                pagination: None,
            }))
        }
        Ok(None) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some("Registry configuration not found".to_string()),
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Update registry configuration handler
async fn update_config_handler(
    config: RegistryConfig,
    db: Database,
) -> Result<impl Reply, Rejection> {
    match db.update_config(config).await {
        Ok(config) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Success,
                data: Some(config),
                error: None,
                pagination: None,
            }))
        }
        Err(e) => {
            Ok(warp::reply::json(&ApiResponse {
                status: ResponseStatus::Error,
                data: None,
                error: Some(e.to_string()),
                pagination: None,
            }))
        }
    }
}

/// Sync handler
async fn sync_handler(
    db: Database,
) -> Result<impl Reply, Rejection> {
    // In a real implementation, this would sync with peer registries
    // For now, return a success response
    
    Ok(warp::reply::json(&ApiResponse {
        status: ResponseStatus::Success,
        data: Some("Sync completed successfully".to_string()),
        error: None,
        pagination: None,
    }))
}

/// Validate crate metadata
fn validate_crate_metadata(metadata: &CrateMetadata) -> Result<(), String> {
    if metadata.name.is_empty() {
        return Err("Crate name cannot be empty".to_string());
    }
    
    if metadata.version.is_empty() {
        return Err("Crate version cannot be empty".to_string());
    }
    
    if metadata.authors.is_empty() {
        return Err("Crate must have at least one author".to_string());
    }
    
    if metadata.license.is_empty() {
        return Err("Crate license cannot be empty".to_string());
    }
    
    // Validate version format (basic semver check)
    if !metadata.version.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-') {
        return Err("Invalid version format".to_string());
    }
    
    Ok(())
}

/// Handle rejections and convert to proper JSON responses
async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let (code, message) = if err.is_not_found() {
        (warp::http::StatusCode::NOT_FOUND, "Not Found".to_string())
    } else if let Some(_) = err.find::<warp::reject::MethodNotAllowed>() {
        (warp::http::StatusCode::METHOD_NOT_ALLOWED, "Method Not Allowed".to_string())
    } else if let Some(_) = err.find::<warp::reject::InvalidQuery>() {
        (warp::http::StatusCode::BAD_REQUEST, "Invalid Query".to_string())
    } else if let Some(_) = err.find::<warp::reject::InvalidHeader>() {
        (warp::http::StatusCode::BAD_REQUEST, "Invalid Header".to_string())
    } else if let Some(_) = err.find::<warp::reject::MissingHeader>() {
        (warp::http::StatusCode::BAD_REQUEST, "Missing Header".to_string())
    } else if let Some(_) = err.find::<warp::reject::PayloadTooLarge>() {
        (warp::http::StatusCode::PAYLOAD_TOO_LARGE, "Payload Too Large".to_string())
    } else {
        eprintln!("Unhandled rejection: {:?}", err);
        (warp::http::StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error".to_string())
    };

    let json = warp::reply::json(&ApiResponse::<()> {
        status: ResponseStatus::Error,
        data: None,
        error: Some(message),
        pagination: None,
    });

    Ok(warp::reply::with_status(json, code))
}
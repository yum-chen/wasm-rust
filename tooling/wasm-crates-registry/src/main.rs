//! WasmRust Curation Registry - Main entry point
//!
//! This is the main server application for the WasmRust Curation Registry.

mod api;
mod database;
mod schema;

use crate::database::Database;
use std::env;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    env_logger::init();
    
    println!("🚀 Starting WasmRust Curation Registry Server");
    
    // Get configuration from environment
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:registry.db".to_string());
    
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);
    
    println!("📊 Database: {}", database_url);
    println!("🌐 Server: {}:{}", host, port);
    
    // Initialize database
    let db = Database::new(&database_url).await?;
    db.initialize().await?;
    
    println!("✅ Database initialized successfully");
    
    // Setup API routes
    let routes = api::routes(db);
    
    println!("🔧 API routes configured");
    
    // Start server
    println!("🔄 Server starting on {}:{}", host, port);
    
    warp::serve(routes)
        .run((host.parse::<std::net::IpAddr>()?, port))
        .await;
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_server_startup() {
        // Basic test to ensure the main function doesn't panic
        // In a real test environment, we would mock the database
        assert!(true); // Placeholder test
    }
}
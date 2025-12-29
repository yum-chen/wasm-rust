//! Database layer for WasmRust Curation Registry
//!
//! This module provides database operations for crate metadata storage and retrieval.

use sqlx::{SqlitePool, sqlite::SqlitePoolOptions, Row};
use crate::schema::{
    CrateMetadata, CrateStatus, CompatibilityLevel, Pagination, RegistryConfig, 
    TestResult, WasmCompatibility, TestType, TestOutcome, TestEnvironment,
};
use std::sync::Arc;
use thiserror::Error;

/// Database errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database connection error: {0}")]
    ConnectionError(#[from] sqlx::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Crate not found: {0}")]
    CrateNotFound(String),
    
    #[error("Configuration not found")]
    ConfigNotFound,
    
    #[error("Invalid data: {0}")]
    InvalidData(String),
}

/// Database wrapper with connection pool
#[derive(Clone)]
pub struct Database {
    pool: Arc<SqlitePool>,
}

impl Database {
    /// Create new database instance with connection pool
    pub async fn new(database_url: &str) -> Result<Self, DatabaseError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;
            
        Ok(Database {
            pool: Arc::new(pool),
        })
    }

    /// Initialize database schema
    pub async fn initialize(&self) -> Result<(), DatabaseError> {
        let queries = [
            // Crate metadata table
            r#"
            CREATE TABLE IF NOT EXISTS crates (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                version TEXT NOT NULL,
                description TEXT,
                authors TEXT NOT NULL,
                license TEXT NOT NULL,
                repository TEXT,
                documentation TEXT,
                keywords TEXT,
                categories TEXT,
                wasm_compatibility TEXT NOT NULL,
                gc_ready BOOLEAN NOT NULL,
                dual_compilation BOOLEAN NOT NULL,
                dependencies TEXT,
                dev_dependencies TEXT,
                build_dependencies TEXT,
                test_results TEXT,
                crate_size INTEGER NOT NULL,
                wasm_size INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                status TEXT NOT NULL,
                signatures TEXT,
                fork_info TEXT,
                UNIQUE(name, version)
            )
            "#,
            
            // Registry configuration table
            r#"
            CREATE TABLE IF NOT EXISTS registry_config (
                id INTEGER PRIMARY KEY,
                config TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            
            // Test results table (separate for better querying)
            r#"
            CREATE TABLE IF NOT EXISTS test_results (
                id INTEGER PRIMARY KEY,
                crate_id TEXT NOT NULL,
                test_results TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (crate_id) REFERENCES crates (id)
            )
            "#,
        ];

        for query in queries {
            sqlx::query(query)
                .execute(&*self.pool)
                .await?;
        }

        Ok(())
    }

    /// Create a new crate in the database
    pub async fn create_crate(&self, crate_metadata: CrateMetadata) -> Result<CrateMetadata, DatabaseError> {
        let query = r#"
            INSERT INTO crates (
                id, name, version, description, authors, license, repository, documentation,
                keywords, categories, wasm_compatibility, gc_ready, dual_compilation,
                dependencies, dev_dependencies, build_dependencies, test_results,
                crate_size, wasm_size, created_at, updated_at, status, signatures, fork_info
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#;

        sqlx::query(query)
            .bind(&crate_metadata.id)
            .bind(&crate_metadata.name)
            .bind(&crate_metadata.version)
            .bind(&crate_metadata.description)
            .bind(serde_json::to_string(&crate_metadata.authors)?)
            .bind(&crate_metadata.license)
            .bind(&crate_metadata.repository)
            .bind(crate_metadata.documentation.as_ref())
            .bind(serde_json::to_string(&crate_metadata.keywords)?)
            .bind(serde_json::to_string(&crate_metadata.categories)?)
            .bind(serde_json::to_string(&crate_metadata.wasm_compatibility)?)
            .bind(crate_metadata.gc_ready)
            .bind(crate_metadata.dual_compilation)
            .bind(serde_json::to_string(&crate_metadata.dependencies)?)
            .bind(serde_json::to_string(&crate_metadata.dev_dependencies)?)
            .bind(serde_json::to_string(&crate_metadata.build_dependencies)?)
            .bind(serde_json::to_string(&crate_metadata.test_results)?)
            .bind(crate_metadata.crate_size as i64)
            .bind(crate_metadata.wasm_size.map(|s| s as i64))
            .bind(crate_metadata.created_at.to_rfc3339())
            .bind(crate_metadata.updated_at.to_rfc3339())
            .bind(serde_json::to_string(&crate_metadata.status)?)
            .bind(serde_json::to_string(&crate_metadata.signatures)?)
            .bind(if let Some(fork_info) = &crate_metadata.fork_info {
                Some(serde_json::to_string(fork_info)?)
            } else {
                None
            })
            .execute(&*self.pool)
            .await?;

        Ok(crate_metadata)
    }

    /// Get a crate by ID
    pub async fn get_crate(&self, crate_id: &str) -> Result<Option<CrateMetadata>, DatabaseError> {
        let query = "SELECT * FROM crates WHERE id = ?";
        
        let row = sqlx::query(query)
            .bind(crate_id)
            .fetch_optional(&*self.pool)
            .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_crate_metadata(row)?)),
            None => Ok(None),
        }
    }

    /// Update a crate
    pub async fn update_crate(&self, crate_id: &str, mut crate_metadata: CrateMetadata) -> Result<CrateMetadata, DatabaseError> {
        let query = r#"
            UPDATE crates SET
                name = ?, version = ?, description = ?, authors = ?, license = ?, repository = ?,
                documentation = ?, keywords = ?, categories = ?, wasm_compatibility = ?,
                gc_ready = ?, dual_compilation = ?, dependencies = ?, dev_dependencies = ?,
                build_dependencies = ?, test_results = ?, crate_size = ?, wasm_size = ?,
                updated_at = ?, status = ?, signatures = ?, fork_info = ?
            WHERE id = ?
        "#;

        let result = sqlx::query(query)
            .bind(&crate_metadata.name)
            .bind(&crate_metadata.version)
            .bind(&crate_metadata.description)
            .bind(serde_json::to_string(&crate_metadata.authors)?)
            .bind(&crate_metadata.license)
            .bind(&crate_metadata.repository)
            .bind(crate_metadata.documentation.as_ref())
            .bind(serde_json::to_string(&crate_metadata.keywords)?)
            .bind(serde_json::to_string(&crate_metadata.categories)?)
            .bind(serde_json::to_string(&crate_metadata.wasm_compatibility)?)
            .bind(crate_metadata.gc_ready)
            .bind(crate_metadata.dual_compilation)
            .bind(serde_json::to_string(&crate_metadata.dependencies)?)
            .bind(serde_json::to_string(&crate_metadata.dev_dependencies)?)
            .bind(serde_json::to_string(&crate_metadata.build_dependencies)?)
            .bind(serde_json::to_string(&crate_metadata.test_results)?)
            .bind(crate_metadata.crate_size as i64)
            .bind(crate_metadata.wasm_size.map(|s| s as i64))
            .bind(crate_metadata.updated_at.to_rfc3339())
            .bind(serde_json::to_string(&crate_metadata.status)?)
            .bind(serde_json::to_string(&crate_metadata.signatures)?)
            .bind(if let Some(fork_info) = &crate_metadata.fork_info {
                Some(serde_json::to_string(fork_info)?)
            } else {
                None
            })
            .bind(crate_id)
            .execute(&*self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::CrateNotFound(crate_id.to_string()));
        }

        // Ensure the ID matches
        crate_metadata.id = crate_id.to_string();
        Ok(crate_metadata)
    }

    /// Delete a crate
    pub async fn delete_crate(&self, crate_id: &str) -> Result<(), DatabaseError> {
        let query = "DELETE FROM crates WHERE id = ?";
        
        let result = sqlx::query(query)
            .bind(crate_id)
            .execute(&*self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::CrateNotFound(crate_id.to_string()));
        }

        Ok(())
    }

    /// List crates with pagination and filtering
    pub async fn list_crates(
        &self, 
        page: u32, 
        page_size: u32, 
        status_filter: Option<String>,
        compatibility_filter: Option<String>,
        search: Option<String>,
    ) -> Result<(Vec<CrateMetadata>, Pagination), DatabaseError> {
        let offset = (page - 1) * page_size;
        
        // Build WHERE clause based on filters
        let mut where_clauses = Vec::new();
        let mut params: Vec<String> = Vec::new();
        
        if let Some(status) = status_filter {
            where_clauses.push("status = ?");
            params.push(status);
        }
        
        if let Some(compatibility) = compatibility_filter {
            where_clauses.push("wasm_compatibility LIKE ?");
            params.push(format!("%\"level\":\"{}\"%", compatibility));
        }
        
        if let Some(search_term) = search {
            where_clauses.push("(name LIKE ? OR description LIKE ? OR keywords LIKE ?)");
            params.push(format!("%{}%", search_term));
            params.push(format!("%{}%", search_term));
            params.push(format!("%{}%", search_term));
        }
        
        let where_clause = if where_clauses.is_empty() {
            "".to_string()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };
        
        // Count total for pagination
        let count_query = format!("SELECT COUNT(*) as count FROM crates {}", where_clause);
        let mut count_stmt = sqlx::query(&count_query);
        
        for param in &params {
            count_stmt = count_stmt.bind(param);
        }
        
        let count_row = count_stmt.fetch_one(&*self.pool).await?;
        let total: i64 = count_row.get("count");
        
        // Get paginated results
        let query = format!(
            "SELECT * FROM crates {} ORDER BY updated_at DESC LIMIT ? OFFSET ?", 
            where_clause
        );
        
        let mut stmt = sqlx::query(&query);
        
        for param in params {
            stmt = stmt.bind(param);
        }
        
        stmt = stmt.bind(page_size as i64).bind(offset as i64);
        
        let rows = stmt.fetch_all(&*self.pool).await?;
        let crates: Result<Vec<CrateMetadata>, _> = rows
            .into_iter()
            .map(|row| self.row_to_crate_metadata(row))
            .collect();
            
        let pagination = Pagination {
            page,
            page_size,
            total: total as u64,
            total_pages: ((total as f64) / (page_size as f64)).ceil() as u32,
        };
        
        Ok((crates?, pagination))
    }

    /// Update test results for a crate
    pub async fn update_test_results(&self, crate_id: &str, test_results: Vec<TestResult>) -> Result<(), DatabaseError> {
        let query = "UPDATE crates SET test_results = ? WHERE id = ?";
        
        let result = sqlx::query(query)
            .bind(serde_json::to_string(&test_results)?)
            .bind(crate_id)
            .execute(&*self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::CrateNotFound(crate_id.to_string()));
        }

        Ok(())
    }

    /// Get test results for a crate
    pub async fn get_test_results(&self, crate_id: &str) -> Result<Option<Vec<TestResult>>, DatabaseError> {
        let query = "SELECT test_results FROM crates WHERE id = ?";
        
        let row = sqlx::query(query)
            .bind(crate_id)
            .fetch_optional(&*self.pool)
            .await?;

        match row {
            Some(row) => {
                let test_results_json: String = row.get("test_results");
                let test_results: Vec<TestResult> = serde_json::from_str(&test_results_json)?;
                Ok(Some(test_results))
            }
            None => Ok(None),
        }
    }

    /// Get registry configuration
    pub async fn get_config(&self) -> Result<Option<RegistryConfig>, DatabaseError> {
        let query = "SELECT config FROM registry_config ORDER BY id DESC LIMIT 1";
        
        let row = sqlx::query(query)
            .fetch_optional(&*self.pool)
            .await?;

        match row {
            Some(row) => {
                let config_json: String = row.get("config");
                let config: RegistryConfig = serde_json::from_str(&config_json)?;
                Ok(Some(config))
            }
            None => Ok(None),
        }
    }

    /// Update registry configuration
    pub async fn update_config(&self, config: RegistryConfig) -> Result<RegistryConfig, DatabaseError> {
        let query = "INSERT INTO registry_config (config, updated_at) VALUES (?, ?)";
        
        sqlx::query(query)
            .bind(serde_json::to_string(&config)?)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(&*self.pool)
            .await?;

        Ok(config)
    }

    /// Convert database row to CrateMetadata
    fn row_to_crate_metadata(&self, row: sqlx::sqlite::SqliteRow) -> Result<CrateMetadata, DatabaseError> {
        Ok(CrateMetadata {
            id: row.get("id"),
            name: row.get("name"),
            version: row.get("version"),
            description: row.get("description"),
            authors: serde_json::from_str(row.get("authors"))?,
            license: row.get("license"),
            repository: row.get("repository"),
            documentation: row.get("documentation"),
            keywords: serde_json::from_str(row.get("keywords"))?,
            categories: serde_json::from_str(row.get("categories"))?,
            wasm_compatibility: serde_json::from_str(row.get("wasm_compatibility"))?,
            gc_ready: row.get("gc_ready"),
            dual_compilation: row.get("dual_compilation"),
            dependencies: serde_json::from_str(row.get("dependencies"))?,
            dev_dependencies: serde_json::from_str(row.get("dev_dependencies"))?,
            build_dependencies: serde_json::from_str(row.get("build_dependencies"))?,
            test_results: serde_json::from_str(row.get("test_results"))?,
            crate_size: row.get::<i64, _>("crate_size") as u64,
            wasm_size: row.get::<Option<i64>, _>("wasm_size").map(|s| s as u64),
            created_at: chrono::DateTime::parse_from_rfc3339(row.get("created_at"))
                .map_err(|e| DatabaseError::InvalidData(e.to_string()))?
                .with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(row.get("updated_at"))
                .map_err(|e| DatabaseError::InvalidData(e.to_string()))?
                .with_timezone(&chrono::Utc),
            status: serde_json::from_str(row.get("status"))?,
            signatures: serde_json::from_str(row.get("signatures"))?,
            fork_info: if let Some(fork_info_json) = row.get::<Option<String>, _>("fork_info") {
                Some(serde_json::from_str(&fork_info_json)?)
            } else {
                None
            },
        })
    }
}
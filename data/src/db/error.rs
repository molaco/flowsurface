use thiserror::Error;

/// Database-specific errors with rich context for debugging and recovery
#[derive(Error, Debug)]
pub enum DatabaseError {
    /// Failed to establish or access database connection
    #[error("Connection error: {0}")]
    Connection(String),

    /// Schema initialization or migration failed
    #[error("Schema error: {0}")]
    Schema(String),

    /// Query execution failed
    #[error("Query error: {0}")]
    Query(String),

    /// Database configuration is invalid
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Lock acquisition failed (threading issue)
    #[error("Lock error: failed to acquire database lock")]
    LockError,

    /// Underlying DuckDB error
    #[error("DuckDB error: {0}")]
    DuckDB(#[from] duckdb::Error),

    /// IO error (file operations, etc.)
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Transaction error
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Insert operation failed
    #[error("Insert error: {0}")]
    Insert(String),

    /// Not found error
    #[error("Not found: {0}")]
    NotFound(String),

    /// Migration error
    #[error("Migration error: {0}")]
    Migration(String),
}

/// Result type alias for database operations
pub type Result<T> = std::result::Result<T, DatabaseError>;

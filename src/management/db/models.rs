use chrono::{DateTime, Utc};
use serde_json;
use sqlx::{
    types::{JsonValue, Uuid},
    FromRow,
};

// Potentially import ProviderType from dto if it's to be used directly here,
// or handle string conversion in the repository layer.

/// Represents a provider configuration record in the database.
#[derive(Debug, sqlx::FromRow)] // sqlx::FromRow for mapping query results
pub struct Provider {
    pub id: Uuid,
    pub name: String,
    pub provider_type: String, // Stored as VARCHAR in DB, maps to ProviderType enum conceptually
    pub config_details: JsonValue, // Stored as JSONB in DB
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)] // Added Clone here for potential use in services
pub struct ModelDefinition {
    pub id: Uuid,
    pub key: String,
    pub model_type: String,
    pub provider_id: Uuid,
    pub config_details: Option<serde_json::Value>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Represents a pipeline record in the database.
#[derive(Debug, FromRow, Clone)]
pub struct Pipeline {
    pub id: Uuid,
    pub name: String,
    pub pipeline_type: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Represents a pipeline plugin configuration record in the database.
#[derive(Debug, FromRow, Clone)]
pub struct PipelinePluginConfig {
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub plugin_type: String,
    pub config_data: serde_json::Value, // Stored as JSONB
    pub enabled: bool,
    pub order_in_pipeline: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Helper struct to combine a Pipeline with its associated plugins.
/// This is not a direct database model but used for service/repository logic.
#[derive(Debug, Clone)]
pub struct PipelineWithPlugins {
    pub id: Uuid,
    pub name: String,
    pub pipeline_type: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub plugins: Vec<PipelinePluginConfig>,
}

use serde_json::Value as JsonValue;
use sqlx::{query, query_as, types::Uuid, PgPool, Result as SqlxResult};

use crate::db::models::Provider;
use crate::dto::{CreateProviderRequest, UpdateProviderRequest}; // Using DTOs

// We might need CreateProviderRequest or similar DTOs if we pass parts of them directly,
// or we pass decomposed values (name, provider_type string, config_details JsonValue).

// For now, let's assume a simplified ProviderData struct or individual params for creation/update.

#[derive(Debug)] // Temporary struct for conveying data, replace with actual DTOs or decomposed params
pub struct CreateProviderData {
    pub name: String,
    pub provider_type: String, // as string, matching DB
    pub config_details: JsonValue,
    pub enabled: bool,
}

#[derive(Debug)] // Temporary struct
pub struct UpdateProviderData {
    pub name: Option<String>,
    pub config_details: Option<JsonValue>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone)] // Added Clone
pub struct ProviderRepository {
    pool: PgPool,
}

impl ProviderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        data: &CreateProviderRequest,
        provider_type_str: &str,
        config_json_value: JsonValue,
    ) -> SqlxResult<Provider> {
        let new_id = Uuid::new_v4(); // SQLx can often handle default UUIDs if schema is set up
        let enabled = data.enabled.unwrap_or(true);
        query_as!(
            Provider,
            r#"
            INSERT INTO hub_llmgateway_ee_providers (id, name, provider_type, config_details, enabled)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, name, provider_type, config_details, enabled, created_at, updated_at
            "#,
            new_id,
            data.name,
            provider_type_str,
            config_json_value,
            enabled
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn find_by_id(&self, id: Uuid) -> SqlxResult<Option<Provider>> {
        query_as!(
            Provider,
            r#"
            SELECT id, name, provider_type, config_details, enabled, created_at, updated_at
            FROM hub_llmgateway_ee_providers
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_by_name(&self, name: &str) -> SqlxResult<Option<Provider>> {
        query_as!(
            Provider,
            r#"
            SELECT id, name, provider_type, config_details, enabled, created_at, updated_at
            FROM hub_llmgateway_ee_providers
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list(&self) -> SqlxResult<Vec<Provider>> {
        query_as!(
            Provider,
            r#"
            SELECT id, name, provider_type, config_details, enabled, created_at, updated_at
            FROM hub_llmgateway_ee_providers
            ORDER BY name
            "#
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update(
        &self,
        id: Uuid,
        data: &UpdateProviderRequest,
        config_json_value_opt: Option<JsonValue>,
    ) -> SqlxResult<Option<Provider>> {
        // Fetch current and merge, or use COALESCE intelligently
        // For simplicity, this query relies on COALESCE for all fields in data.
        // If a field in `data` is None, COALESCE will keep the existing DB value.
        // If config_json_value_opt is None, it means config is not being updated.
        // If config_json_value_opt is Some(JsonValue::Null), it means clear it.
        // If config_json_value_opt is Some(ActualValue), it means update it.

        // We need to handle config_details carefully because COALESCE won't distinguish between
        // not providing the field (keep current) vs. providing null (set to null).
        // The current query_as! macro might not easily support conditional SET clauses.
        // A more robust way would be to build the query string dynamically or fetch and merge, then save.
        // For now, let's assume service layer prepares `config_json_value_opt` to be Some(value) or None (meaning no change to config_details).
        // And if user wants to set config_details to NULL, they'd pass Some(JsonValue::Null)

        let current_provider = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        let name_to_update = data.name.as_ref().unwrap_or(&current_provider.name);
        let enabled_to_update = data.enabled.unwrap_or(current_provider.enabled);

        let final_config_details: JsonValue = match config_json_value_opt {
            Some(new_val) => new_val,                        // new_val is JsonValue
            None => current_provider.config_details.clone(), // current_provider.config_details is JsonValue, clone it
        };

        query_as!(
            Provider,
            r#"
            UPDATE hub_llmgateway_ee_providers
            SET
                name = $1,
                config_details = $2,
                enabled = $3,
                updated_at = now()
            WHERE id = $4
            RETURNING id, name, provider_type, config_details, enabled, created_at, updated_at
            "#,
            name_to_update,
            final_config_details, // This is Option<JsonValue>
            enabled_to_update,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete(&self, id: Uuid) -> SqlxResult<u64> {
        let result = query!(
            r#"
            DELETE FROM hub_llmgateway_ee_providers
            WHERE id = $1
            "#,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

use crate::management::{
    db::models::ModelDefinition,
    dto::{CreateModelDefinitionRequest, UpdateModelDefinitionRequest},
};
use sqlx::{PgPool, Result, query, query_as, types::Uuid};

#[derive(Debug, Clone)]
pub struct ModelDefinitionRepository {
    pool: PgPool,
}

impl ModelDefinitionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, data: &CreateModelDefinitionRequest) -> Result<ModelDefinition> {
        let default_enabled = data.enabled.unwrap_or(true);
        let model_def = query_as!(ModelDefinition,
            r#"
            INSERT INTO hub_llmgateway_model_definitions (key, model_type, provider_id, config_details, enabled)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, key, model_type, provider_id, config_details, enabled, created_at, updated_at
            "#,
            data.key,
            data.model_type,
            data.provider_id,
            data.config_details.as_ref().map(|val| val.clone()), // Option<Value> -> Option<Value>
            default_enabled
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(model_def)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<ModelDefinition>> {
        query_as!(ModelDefinition,
            "SELECT id, key, model_type, provider_id, config_details, enabled, created_at, updated_at FROM hub_llmgateway_model_definitions WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn find_by_key(&self, key: &str) -> Result<Option<ModelDefinition>> {
        query_as!(ModelDefinition,
            "SELECT id, key, model_type, provider_id, config_details, enabled, created_at, updated_at FROM hub_llmgateway_model_definitions WHERE key = $1",
            key
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list(&self) -> Result<Vec<ModelDefinition>> {
        query_as!(ModelDefinition, "SELECT id, key, model_type, provider_id, config_details, enabled, created_at, updated_at FROM hub_llmgateway_model_definitions ORDER BY key ASC")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn update(
        &self,
        id: Uuid,
        data: &UpdateModelDefinitionRequest,
    ) -> Result<ModelDefinition> {
        // Fetch current to handle Option fields correctly
        let current_model = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| sqlx::Error::RowNotFound)?;

        let key = data.key.as_ref().unwrap_or(&current_model.key);
        let model_type = data
            .model_type
            .as_ref()
            .unwrap_or(&current_model.model_type);
        let provider_id = data.provider_id.unwrap_or(current_model.provider_id);
        let enabled = data.enabled.unwrap_or(current_model.enabled);

        // For config_details, if Option is Some(Value), update. If Some(Null), set to NULL. If None, keep current.
        let config_details_to_update = match data.config_details.as_ref() {
            Some(serde_json::Value::Null) => None, // Explicitly set to NULL
            Some(value) => Some(value.clone()),    // Update with new value
            None => current_model.config_details.clone(), // Keep existing value
        };

        let model_def = query_as!(ModelDefinition,
            r#"
            UPDATE hub_llmgateway_model_definitions
            SET key = $1, model_type = $2, provider_id = $3, config_details = $4, enabled = $5, updated_at = NOW()
            WHERE id = $6
            RETURNING id, key, model_type, provider_id, config_details, enabled, created_at, updated_at
            "#,
            key,
            model_type,
            provider_id,
            config_details_to_update,
            enabled,
            id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(model_def)
    }

    pub async fn delete(&self, id: Uuid) -> Result<u64> {
        let result = query!(
            "DELETE FROM hub_llmgateway_model_definitions WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

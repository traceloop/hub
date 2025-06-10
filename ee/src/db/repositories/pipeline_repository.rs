use sqlx::{query_as, types::Uuid, PgPool, Row};
use std::collections::HashMap;

use crate::{
    db::models::{Pipeline, PipelinePluginConfig, PipelineWithPlugins},
    dto::{CreatePipelineRequestDto, UpdatePipelineRequestDto},
    errors::ApiError,
};

// Temporary internal structs for repository data, might be refined or replaced by DTOs directly if suitable
// For now, using DTOs directly in method signatures where it makes sense.

#[derive(Debug)]
pub struct PipelineRepository {
    pool: PgPool,
}

impl PipelineRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_pipeline_with_plugins(
        &self,
        pipeline_data: &CreatePipelineRequestDto,
    ) -> Result<PipelineWithPlugins, ApiError> {
        let mut tx = self.pool.begin().await.map_err(ApiError::from)?;

        let pipeline = query_as!(
            Pipeline,
            r#"
            INSERT INTO hub_llmgateway_ee_pipelines (name, pipeline_type, description, enabled)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, pipeline_type, description, enabled, created_at, updated_at
            "#,
            pipeline_data.name,
            pipeline_data.pipeline_type,
            pipeline_data.description,
            pipeline_data.enabled
        )
        .fetch_one(&mut *tx) // Use &mut *tx for transaction
        .await
        .map_err(ApiError::from)?;

        let mut created_plugins: Vec<PipelinePluginConfig> = Vec::new();
        if !pipeline_data.plugins.is_empty() {
            for plugin_dto in &pipeline_data.plugins {
                let plugin_config = query_as!(PipelinePluginConfig,
                    r#"
                    INSERT INTO hub_llmgateway_ee_pipeline_plugin_configs 
                        (pipeline_id, plugin_type, config_data, enabled, order_in_pipeline)
                    VALUES ($1, $2, $3, $4, $5)
                    RETURNING id, pipeline_id, plugin_type, config_data, enabled, order_in_pipeline, created_at, updated_at
                    "#,
                    pipeline.id,
                    plugin_dto.plugin_type.to_string(),
                    plugin_dto.config_data, // Assuming config_data is already a serde_json::Value
                    plugin_dto.enabled,
                    plugin_dto.order_in_pipeline
                )
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| ApiError::from(e))?;
                created_plugins.push(plugin_config);
            }
        }

        tx.commit().await.map_err(|e| ApiError::from(e))?;

        Ok(PipelineWithPlugins {
            id: pipeline.id,
            name: pipeline.name,
            pipeline_type: pipeline.pipeline_type,
            description: pipeline.description,
            enabled: pipeline.enabled,
            created_at: pipeline.created_at,
            updated_at: pipeline.updated_at,
            plugins: created_plugins,
        })
    }

    pub async fn find_pipeline_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<PipelineWithPlugins>, ApiError> {
        let pipeline_row = sqlx::query!(
            r#"
            SELECT id, name, pipeline_type, description, enabled, created_at, updated_at
            FROM hub_llmgateway_ee_pipelines
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::from(e))?;

        if let Some(row) = pipeline_row {
            let plugins = sqlx::query_as!(
                PipelinePluginConfig,
                r#"
                SELECT id, pipeline_id, plugin_type, config_data, enabled, order_in_pipeline, created_at, updated_at
                FROM hub_llmgateway_ee_pipeline_plugin_configs
                WHERE pipeline_id = $1
                ORDER BY order_in_pipeline ASC
                "#,
                row.id
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ApiError::from(e))?;

            Ok(Some(PipelineWithPlugins {
                id: row.id,
                name: row.name,
                pipeline_type: row.pipeline_type,
                description: row.description,
                enabled: row.enabled,
                created_at: row.created_at,
                updated_at: row.updated_at,
                plugins,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn find_pipeline_by_name(
        &self,
        name: &str,
    ) -> Result<Option<PipelineWithPlugins>, ApiError> {
        let pipeline_row = sqlx::query!(
            r#"
            SELECT id, name, pipeline_type, description, enabled, created_at, updated_at
            FROM hub_llmgateway_ee_pipelines
            WHERE name = $1
            "#,
            name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ApiError::from(e))?;

        if let Some(row) = pipeline_row {
            let plugins = sqlx::query_as!(
                PipelinePluginConfig,
                r#"
                SELECT id, pipeline_id, plugin_type, config_data, enabled, order_in_pipeline, created_at, updated_at
                FROM hub_llmgateway_ee_pipeline_plugin_configs
                WHERE pipeline_id = $1
                ORDER BY order_in_pipeline ASC
                "#,
                row.id
            )
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ApiError::from(e))?;

            Ok(Some(PipelineWithPlugins {
                id: row.id,
                name: row.name,
                pipeline_type: row.pipeline_type,
                description: row.description,
                enabled: row.enabled,
                created_at: row.created_at,
                updated_at: row.updated_at,
                plugins,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_pipelines(&self) -> Result<Vec<PipelineWithPlugins>, ApiError> {
        let pipelines = query_as!(
            Pipeline,
            r#"
            SELECT id, name, pipeline_type, description, enabled, created_at, updated_at
            FROM hub_llmgateway_ee_pipelines
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::from(e))?;

        if pipelines.is_empty() {
            return Ok(Vec::new());
        }

        let pipeline_ids: Vec<Uuid> = pipelines.iter().map(|p| p.id).collect();

        let all_plugins = sqlx::query_as!(
            PipelinePluginConfig,
            r#"
            SELECT id, pipeline_id, plugin_type, config_data, enabled, order_in_pipeline, created_at, updated_at
            FROM hub_llmgateway_ee_pipeline_plugin_configs
            WHERE pipeline_id = ANY($1)
            ORDER BY pipeline_id, order_in_pipeline ASC
            "#,
            &pipeline_ids // Pass as slice
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApiError::from(e))?;

        let mut plugins_map: HashMap<Uuid, Vec<PipelinePluginConfig>> = HashMap::new();
        for plugin in all_plugins {
            plugins_map
                .entry(plugin.pipeline_id)
                .or_default()
                .push(plugin);
        }

        let result = pipelines
            .into_iter()
            .map(|p| PipelineWithPlugins {
                id: p.id,
                name: p.name.clone(),
                pipeline_type: p.pipeline_type.clone(),
                description: p.description.clone(),
                enabled: p.enabled,
                created_at: p.created_at,
                updated_at: p.updated_at,
                plugins: plugins_map.remove(&p.id).unwrap_or_default(),
            })
            .collect();

        Ok(result)
    }

    pub async fn update_pipeline(
        &self,
        id: Uuid,
        data: &UpdatePipelineRequestDto,
    ) -> Result<PipelineWithPlugins, ApiError> {
        let mut tx = self.pool.begin().await.map_err(|e| ApiError::from(e))?;

        // Fetch current pipeline to check existence and for returning non-updated fields
        let current_pipeline = sqlx::query_as!(
            Pipeline,
            "SELECT * FROM hub_llmgateway_ee_pipelines WHERE id = $1",
            id
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| ApiError::from(e))?
        .ok_or(ApiError::NotFound("Pipeline not found".to_string()))?;

        let updated_pipeline = query_as!(
            Pipeline,
            r#"
            UPDATE hub_llmgateway_ee_pipelines
            SET 
                name = COALESCE($1, name),
                pipeline_type = COALESCE($2, pipeline_type),
                description = COALESCE($3, description),
                enabled = COALESCE($4, enabled),
                updated_at = NOW()
            WHERE id = $5
            RETURNING id, name, pipeline_type, description, enabled, created_at, updated_at
            "#,
            data.name.as_ref().unwrap_or(&current_pipeline.name),
            data.pipeline_type
                .as_ref()
                .unwrap_or(&current_pipeline.pipeline_type),
            data.description
                .as_ref()
                .or(current_pipeline.description.as_ref()), // Handles Option<String>
            data.enabled.unwrap_or(current_pipeline.enabled),
            id
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| ApiError::from(e))?;

        let mut updated_plugins_list: Vec<PipelinePluginConfig> = Vec::new();

        if let Some(plugins_dto_list) = &data.plugins {
            // Delete existing plugins for this pipeline
            sqlx::query!(
                "DELETE FROM hub_llmgateway_ee_pipeline_plugin_configs WHERE pipeline_id = $1",
                id
            )
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::from(e))?;

            // Insert new plugins
            for plugin_dto in plugins_dto_list {
                let new_plugin = query_as!(PipelinePluginConfig,
                    r#"
                    INSERT INTO hub_llmgateway_ee_pipeline_plugin_configs 
                        (pipeline_id, plugin_type, config_data, enabled, order_in_pipeline)
                    VALUES ($1, $2, $3, $4, $5)
                    RETURNING id, pipeline_id, plugin_type, config_data, enabled, order_in_pipeline, created_at, updated_at
                    "#,
                    updated_pipeline.id,
                    plugin_dto.plugin_type.to_string(),
                    plugin_dto.config_data,
                    plugin_dto.enabled,
                    plugin_dto.order_in_pipeline
                )
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| ApiError::from(e))?;
                updated_plugins_list.push(new_plugin);
            }
        } else {
            // If plugins field is not provided in the update DTO, retain existing plugins
            let existing_plugins = sqlx::query_as!(
                PipelinePluginConfig,
                r#"
                SELECT id, pipeline_id, plugin_type, config_data, enabled, order_in_pipeline, created_at, updated_at
                FROM hub_llmgateway_ee_pipeline_plugin_configs
                WHERE pipeline_id = $1
                ORDER BY order_in_pipeline ASC
                "#,
                id
            )
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| ApiError::from(e))?;
            updated_plugins_list = existing_plugins;
        }

        tx.commit().await.map_err(|e| ApiError::from(e))?;

        Ok(PipelineWithPlugins {
            id: updated_pipeline.id,
            name: updated_pipeline.name,
            pipeline_type: updated_pipeline.pipeline_type,
            description: updated_pipeline.description,
            enabled: updated_pipeline.enabled,
            created_at: updated_pipeline.created_at, // This should be original creation time
            updated_at: updated_pipeline.updated_at,
            plugins: updated_plugins_list,
        })
    }

    pub async fn delete_pipeline(&self, id: Uuid) -> Result<u64, ApiError> {
        // The `ON DELETE CASCADE` constraint on `pipeline_plugin_configs.pipeline_id`
        // should handle deleting associated plugins automatically.
        let result = sqlx::query!("DELETE FROM hub_llmgateway_ee_pipelines WHERE id = $1", id)
            .execute(&self.pool)
            .await
            .map_err(|e| ApiError::from(e))?;

        if result.rows_affected() == 0 {
            return Err(ApiError::NotFound("Pipeline not found".to_string()));
        }
        Ok(result.rows_affected())
    }

    /// Checks if all provided model definition keys exist in the database.
    /// Returns Ok(true) if all exist, Ok(false) if any do not exist, or an ApiError.
    pub async fn check_model_definition_keys_exist(
        &self,
        keys: &[String],
    ) -> Result<bool, ApiError> {
        if keys.is_empty() {
            return Ok(true); // No keys to check, so they all "exist"
        }

        // Construct a query like: SELECT key FROM hub_llmgateway_ee_model_definitions WHERE key = ANY($1)
        // This is more efficient than querying one by one.
        let existing_keys: Vec<String> =
            sqlx::query("SELECT key FROM hub_llmgateway_ee_model_definitions WHERE key = ANY($1)")
                .bind(keys)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ApiError::from(e))?
                .into_iter()
                .map(|row| row.get("key"))
                .collect();

        // Check if the count of found keys matches the count of input keys
        Ok(existing_keys.len() == keys.len())
    }
}

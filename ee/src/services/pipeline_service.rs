use std::sync::Arc;

use sqlx::types::Uuid;

use crate::{
    db::models::PipelineWithPlugins, // Internal struct from repository
    // We'll need ModelDefinitionRepository for validating model keys in model-router
    db::repositories::model_definition_repository::ModelDefinitionRepository,
    db::repositories::pipeline_repository::PipelineRepository,
    dto::{
        CreatePipelineRequestDto, ModelRouterConfigDto, PipelinePluginConfigDto,
        PipelineResponseDto, PluginType, UpdatePipelineRequestDto,
    },
    errors::ApiError,
};

#[derive(Debug)]
pub struct PipelineService {
    repo: Arc<PipelineRepository>,
    model_definition_repo: Arc<ModelDefinitionRepository>, // For validation
}

impl PipelineService {
    pub fn new(
        repo: Arc<PipelineRepository>,
        model_definition_repo: Arc<ModelDefinitionRepository>,
    ) -> Self {
        Self {
            repo,
            model_definition_repo,
        }
    }

    // Helper to map PipelineWithPlugins to PipelineResponseDto
    fn map_db_pipeline_to_response(
        &self,
        db_pipeline: PipelineWithPlugins,
    ) -> Result<PipelineResponseDto, ApiError> {
        let mut plugin_dtos: Vec<PipelinePluginConfigDto> = Vec::new();
        for plugin_config in db_pipeline.plugins {
            // Here, we might want to validate/deserialize config_data if it's for model-router
            // For now, just pass it through as Value. Deserialization to specific types like
            // ModelRouterConfigDto can happen at the point of use or if strict typing is needed in response.
            let plugin_type = plugin_config
                .plugin_type
                .parse::<PluginType>()
                .map_err(|e| {
                    ApiError::InternalServerError(format!("Invalid plugin type in database: {e}"))
                })?;

            plugin_dtos.push(PipelinePluginConfigDto {
                plugin_type,
                config_data: plugin_config.config_data,
                enabled: plugin_config.enabled,
                order_in_pipeline: plugin_config.order_in_pipeline,
            });
        }

        Ok(PipelineResponseDto {
            id: db_pipeline.id,
            name: db_pipeline.name,
            pipeline_type: db_pipeline.pipeline_type,
            description: db_pipeline.description,
            plugins: plugin_dtos,
            enabled: db_pipeline.enabled,
            created_at: db_pipeline.created_at,
            updated_at: db_pipeline.updated_at,
        })
    }

    // Renamed and focused for creation scenarios
    async fn validate_pipeline_for_creation(
        &self,
        name: &str,
        plugins: &[PipelinePluginConfigDto],
    ) -> Result<(), ApiError> {
        // Validate pipeline name uniqueness for new pipelines
        if self.repo.find_pipeline_by_name(name).await?.is_some() {
            return Err(ApiError::Conflict(format!(
                "Pipeline name '{name}' already exists"
            )));
        }
        // Validate plugin configurations
        self.validate_plugins_config(plugins).await
    }

    // New method specifically for validating plugin configurations
    async fn validate_plugins_config(
        &self,
        plugins: &[PipelinePluginConfigDto],
    ) -> Result<(), ApiError> {
        for plugin_dto in plugins {
            if plugin_dto.plugin_type == PluginType::ModelRouter {
                let model_router_config: ModelRouterConfigDto =
                    serde_json::from_value(plugin_dto.config_data.clone()).map_err(|e| {
                        ApiError::ValidationError(format!("Invalid model-router config_data: {e}"))
                    })?;
                for model_entry in model_router_config.models {
                    // Assuming model_definition_repo.find_by_key now doesn't need PgPool
                    // If it does, it needs to be passed or self.model_definition_repo needs to hold the pool
                    // For now, let's assume it's available or adapted.
                    // This highlights a potential dependency issue if ModelDefinitionRepository needs a pool per call
                    // For now, we'll keep the existing call structure, assuming find_by_key is adaptable
                    // or the repo instance has access to a pool.
                    // To make this compile, we need to ensure find_by_key can be called.
                    // Let's assume it does not need the pool directly for this example
                    // and that its internal state or a shared pool is used.
                    // THIS IS A PLACEHOLDER - ModelDefinitionRepository interaction needs verification
                    if self
                        .model_definition_repo
                        .find_by_key(&model_entry.key)
                        .await?
                        .is_none()
                    {
                        return Err(ApiError::ValidationError(format!(
                            "ModelDefinition key '{}' not found for model-router",
                            model_entry.key
                        )));
                    }
                }
            }
            // Add other plugin type validations here if necessary
        }
        Ok(())
    }

    pub async fn create_pipeline(
        &self,
        request: CreatePipelineRequestDto,
    ) -> Result<PipelineResponseDto, ApiError> {
        // Use the more specific validation method for creation
        self.validate_pipeline_for_creation(&request.name, &request.plugins)
            .await?;
        let created_db_pipeline = self.repo.create_pipeline_with_plugins(&request).await?;
        self.map_db_pipeline_to_response(created_db_pipeline)
    }

    pub async fn get_pipeline(&self, id: Uuid) -> Result<PipelineResponseDto, ApiError> {
        let db_pipeline = self.repo.find_pipeline_by_id(id).await?;
        match db_pipeline {
            Some(p) => self.map_db_pipeline_to_response(p),
            None => Err(ApiError::NotFound(format!(
                "Pipeline with ID {id} not found"
            ))),
        }
    }

    pub async fn get_pipeline_by_name(&self, name: &str) -> Result<PipelineResponseDto, ApiError> {
        let db_pipeline = self.repo.find_pipeline_by_name(name).await?;
        match db_pipeline {
            Some(p) => self.map_db_pipeline_to_response(p),
            None => Err(ApiError::NotFound(format!(
                "Pipeline with name '{name}' not found"
            ))),
        }
    }

    pub async fn list_pipelines(&self) -> Result<Vec<PipelineResponseDto>, ApiError> {
        let db_pipelines = self.repo.list_pipelines().await?;
        db_pipelines
            .into_iter()
            .map(|p| self.map_db_pipeline_to_response(p))
            .collect()
    }

    pub async fn update_pipeline(
        &self,
        id: Uuid,
        request: UpdatePipelineRequestDto,
    ) -> Result<PipelineResponseDto, ApiError> {
        // Ensure pipeline exists before update
        let existing_pipeline_opt = self.repo.find_pipeline_by_id(id).await?;
        if existing_pipeline_opt.is_none() {
            return Err(ApiError::NotFound(format!(
                "Pipeline with ID {id} not found for update"
            )));
        }

        // Validate new name uniqueness if name is being changed
        if let Some(new_name) = &request.name {
            if let Some(found_pipeline_by_name) = self.repo.find_pipeline_by_name(new_name).await? {
                if found_pipeline_by_name.id != id {
                    // It's a different pipeline with the same new name
                    return Err(ApiError::Conflict(format!(
                        "Pipeline name '{new_name}' already exists"
                    )));
                }
            }
        }

        // Only validate plugins if they are provided in the request
        if let Some(plugins) = &request.plugins {
            self.validate_plugins_config(plugins).await?;
        }
        let updated_db_pipeline = self.repo.update_pipeline(id, &request).await?;
        self.map_db_pipeline_to_response(updated_db_pipeline)
    }

    pub async fn delete_pipeline(&self, id: Uuid) -> Result<(), ApiError> {
        let affected_rows = self.repo.delete_pipeline(id).await?;
        if affected_rows == 0 {
            return Err(ApiError::NotFound(format!(
                "Pipeline with ID {id} not found for deletion"
            )));
        }
        Ok(())
    }
}

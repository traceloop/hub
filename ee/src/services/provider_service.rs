use sqlx::{types::Uuid, PgPool};
use std::sync::Arc;

use crate::{
    db::{models::Provider as DbProvider, repositories::provider_repository::ProviderRepository},
    dto::{
        CreateProviderRequest, ProviderConfig, ProviderResponse, ProviderType,
        UpdateProviderRequest,
    },
    errors::ApiError,
};

#[derive(Clone)]
pub struct ProviderService {
    repo: Arc<ProviderRepository>,
}

impl ProviderService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: Arc::new(ProviderRepository::new(pool)),
        }
    }

    pub async fn create_provider(
        &self,
        request: CreateProviderRequest,
    ) -> Result<ProviderResponse, ApiError> {
        if self.repo.find_by_name(&request.name).await?.is_some() {
            return Err(ApiError::Conflict(format!(
                "Provider with name '{}' already exists.",
                request.name
            )));
        }

        let provider_type_string_for_db = request.provider_type.to_string();

        let config_json_value = serde_json::to_value(&request.config)?;

        let db_provider = self
            .repo
            .create(&request, &provider_type_string_for_db, config_json_value)
            .await?;
        Self::map_db_provider_to_response(db_provider)
    }

    pub async fn get_provider(&self, id: Uuid) -> Result<ProviderResponse, ApiError> {
        let db_provider = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Provider with ID {} not found.", id)))?;
        Self::map_db_provider_to_response(db_provider)
    }

    pub async fn list_providers(&self) -> Result<Vec<ProviderResponse>, ApiError> {
        let db_providers = self.repo.list().await?;
        db_providers
            .into_iter()
            .map(Self::map_db_provider_to_response)
            .collect()
    }

    pub async fn update_provider(
        &self,
        id: Uuid,
        request: UpdateProviderRequest,
    ) -> Result<ProviderResponse, ApiError> {
        let existing_provider = self.repo.find_by_id(id).await?.ok_or_else(|| {
            ApiError::NotFound(format!("Provider with ID {} not found to update.", id))
        })?;

        if let Some(new_name) = &request.name {
            if new_name != &existing_provider.name
                && self.repo.find_by_name(new_name).await?.is_some()
            {
                return Err(ApiError::Conflict(format!(
                    "Another provider with name '{}' already exists.",
                    new_name
                )));
            }
        }

        let config_json_value_opt = match request.config.as_ref() {
            Some(provider_config_enum_instance) => {
                Some(serde_json::to_value(provider_config_enum_instance)?)
            }
            None => None,
        };

        let updated_db_provider = self
            .repo
            .update(id, &request, config_json_value_opt)
            .await?
            .ok_or_else(|| {
                ApiError::NotFound(format!(
                    "Provider with ID {} not found after update attempt.",
                    id
                ))
            })?;

        Self::map_db_provider_to_response(updated_db_provider)
    }

    pub async fn delete_provider(&self, id: Uuid) -> Result<(), ApiError> {
        let affected_rows = self.repo.delete(id).await?;
        if affected_rows == 0 {
            Err(ApiError::NotFound(format!(
                "Provider with ID {} not found, nothing deleted.",
                id
            )))
        } else {
            Ok(())
        }
    }

    fn map_db_provider_to_response(db_provider: DbProvider) -> Result<ProviderResponse, ApiError> {
        let provider_type_enum: ProviderType = db_provider.provider_type.parse().map_err(|e| {
            ApiError::InternalServerError(format!(
                "Failed to parse provider_type '{}' from DB: {}",
                db_provider.provider_type, e
            ))
        })?;

        let config_enum = match provider_type_enum {
            ProviderType::OpenAI => {
                ProviderConfig::OpenAI(serde_json::from_value(db_provider.config_details.clone())?)
            }
            ProviderType::Azure => {
                ProviderConfig::Azure(serde_json::from_value(db_provider.config_details.clone())?)
            }
            ProviderType::Bedrock => {
                ProviderConfig::Bedrock(serde_json::from_value(db_provider.config_details.clone())?)
            }
        };

        Ok(ProviderResponse {
            id: db_provider.id,
            name: db_provider.name,
            provider_type: provider_type_enum,
            config: config_enum,
            enabled: db_provider.enabled,
            created_at: db_provider.created_at,
            updated_at: db_provider.updated_at,
        })
    }
}

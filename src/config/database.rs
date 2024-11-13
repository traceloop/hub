use sqlx::PgPool;
use crate::models::{Model, Provider, Pipeline};

pub struct DatabaseConfig {
    pool: PgPool,
}

impl DatabaseConfig {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn load_providers(&self) -> Result<Vec<Provider>, sqlx::Error> {
        sqlx::query_as!(
            Provider,
            "SELECT name, provider_type, config FROM providers"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn load_models(&self) -> Result<Vec<Model>, sqlx::Error> {
        sqlx::query_as!(
            Model,
            "SELECT name, provider_name, config FROM models"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn load_pipelines(&self) -> Result<Vec<Pipeline>, sqlx::Error> {
        sqlx::query_as!(
            Pipeline,
            "SELECT name, config FROM pipelines"
        )
        .fetch_all(&self.pool)
        .await
    }
} 
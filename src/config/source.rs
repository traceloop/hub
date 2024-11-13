use std::env;

#[derive(Debug, Clone)]
pub enum ConfigSource {
    File(String),
    Database(String),
}

impl ConfigSource {
    pub fn from_env() -> Self {
        match env::var("CONFIG_SOURCE").as_deref() {
            Ok("database") => ConfigSource::Database(
                env::var("DATABASE_URL")
                    .expect("DATABASE_URL must be set when using database config source")
            ),
            _ => ConfigSource::File(
                env::var("CONFIG_FILE")
                    .unwrap_or_else(|_| "config.yaml".to_string())
            ),
        }
    }
} 
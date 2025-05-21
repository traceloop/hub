use std::sync::OnceLock;

use crate::config::models::Config;

pub static TRACE_CONTENT_ENABLED: OnceLock<bool> = OnceLock::new();

pub fn load_config(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
    let contents = std::fs::read_to_string(path)?;
    let config: Config = serde_yaml::from_str(&contents)?;
    TRACE_CONTENT_ENABLED
        .set(config.general.trace_content_enabled)
        .expect("Failed to set trace content enabled flag");
    Ok(config)
}

pub fn get_trace_content_enabled() -> bool {
    *TRACE_CONTENT_ENABLED.get_or_init(|| true)
}

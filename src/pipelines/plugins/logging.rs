use crate::pipelines::plugin::Plugin;
use crate::config::models::PluginConfig;

pub struct LoggingPlugin;

impl Plugin for LoggingPlugin {
    fn name(&self) -> String {
        "logging".to_string()
    }

    fn enabled(&self) -> bool {
        true
    }

    fn init(&mut self, _config: &PluginConfig) -> () {

    }

    fn clone_box(&self) -> Box<dyn Plugin> {
        Box::new(LoggingPlugin)
    }
}

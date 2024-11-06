use crate::config::models::PluginConfig;
use crate::pipelines::plugin::Plugin;

pub struct TracingPlugin;

impl Plugin for TracingPlugin {
    fn name(&self) -> String {
        "tracing".to_string()
    }

    fn enabled(&self) -> bool {
        true
    }

    fn init(&mut self, _config: &PluginConfig) -> () {}

    fn clone_box(&self) -> Box<dyn Plugin> {
        Box::new(TracingPlugin)
    }
}

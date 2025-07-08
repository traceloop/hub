// This file now re-exports types from the shared types module.
// The original struct definitions have been moved to src/types to allow both
// the main gateway and the management crate to use the same configuration structures.

pub use crate::types::*;
// All old struct definitions (Config, Provider, ModelConfig, PipelineType, Pipeline, PluginConfig)
// and their helper functions (default_log_level, no_api_key) are removed from this file.
// They are now defined in the src/types module.

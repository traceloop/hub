// This file now re-exports types from the shared hub_gateway_core_types crate.
// The original struct definitions have been moved there to allow both
// the main gateway and the EE crate to use the same configuration structures.

pub use hub_gateway_core_types::*;
// All old struct definitions (Config, Provider, ModelConfig, PipelineType, Pipeline, PluginConfig)
// and their helper functions (default_log_level, no_api_key) are removed from this file.
// They are now defined in the hub_gateway_core_types crate.

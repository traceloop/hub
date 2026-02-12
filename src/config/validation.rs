use crate::types::GatewayConfig;
use std::collections::HashSet;

/// Validates the logical consistency of a GatewayConfig.
/// Returns Ok(()) if valid, or Err(Vec<String>) with a list of error messages if invalid.
pub fn validate_gateway_config(config: &GatewayConfig) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    // Check 1: Provider references in Models must exist
    let provider_keys: HashSet<&String> = config.providers.iter().map(|p| &p.key).collect();
    for model in &config.models {
        if !provider_keys.contains(&model.provider) {
            errors.push(format!(
                "Model '{}' references non-existent provider '{}'.",
                model.key, model.provider
            ));
        }
    }

    // Check 2: Model references in Pipeline ModelRouters must exist
    let model_keys: HashSet<&String> = config.models.iter().map(|m| &m.key).collect();
    for pipeline in &config.pipelines {
        for plugin in &pipeline.plugins {
            if let crate::types::PluginConfig::ModelRouter {
                models: router_models,
            } = plugin
            {
                for model_key in router_models {
                    if !model_keys.contains(model_key) {
                        errors.push(format!(
                            "Pipeline '{}'s ModelRouter references non-existent model '{}'.",
                            pipeline.name, model_key
                        ));
                    }
                }
            }
        }
    }

    // Check 3: Guardrails validation
    if let Some(gr_config) = &config.guardrails {
       // Guard provider references must exist in guardrails.providers
        for guard in &gr_config.guards {
            if !gr_config.providers.contains_key(&guard.provider) {
                errors.push(format!(
                    "Guard '{}' references non-existent guardrail provider '{}'.",
                    guard.name, guard.provider
                ));
            }
        }

        // Pipeline guard references must exist in guardrails.guards
        let guard_names: HashSet<&String> = gr_config.guards.iter().map(|g| &g.name).collect();
        for pipeline in &config.pipelines {
            for guard_name in &pipeline.guards {
                if !guard_names.contains(guard_name) {
                    errors.push(format!(
                        "Pipeline '{}' references non-existent guard '{}'.",
                        pipeline.name, guard_name
                    ));
                }
            }
        }

        // Guards must have api_base and api_key (either directly or via provider)
        for guard in &gr_config.guards {
            let has_api_base = guard.api_base.as_ref().is_some_and(|s| !s.is_empty())
                || gr_config.providers.get(&guard.provider)
                    .is_some_and(|p| !p.api_base.is_empty());
            let has_api_key = guard.api_key.as_ref().is_some_and(|s| !s.is_empty())
                || gr_config.providers.get(&guard.provider)
                    .is_some_and(|p| !p.api_key.is_empty());

            if !has_api_base {
                errors.push(format!(
                    "Guard '{}' has no api_base configured (neither on the guard nor on provider '{}').",
                    guard.name, guard.provider
                ));
            }
            if !has_api_key {
                errors.push(format!(
                    "Guard '{}' has no api_key configured (neither on the guard nor on provider '{}').",
                    guard.name, guard.provider
                ));
            }
        }

        // Guard names must be unique
        let mut seen_guard_names = HashSet::new();
        for guard in &gr_config.guards {
            if !seen_guard_names.insert(&guard.name) {
                errors.push(format!("Duplicate guard name: '{}'.", guard.name));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*; // To import validate_gateway_config
    use std::collections::HashMap;
    use crate::guardrails::types::{Guard, GuardMode, GuardrailsConfig, OnFailure, ProviderConfig as GrProviderConfig};
    use crate::types::{ModelConfig, Pipeline, PipelineType, PluginConfig, Provider, ProviderType}; // For test data

    #[test]
    fn test_valid_config() {
        let config = GatewayConfig {
            guardrails: None,
            general: None,
            providers: vec![Provider {
                key: "p1".to_string(),
                r#type: ProviderType::OpenAI,
                api_key: "key1".to_string(),
                params: Default::default(),
            }],
            models: vec![ModelConfig {
                key: "m1".to_string(),
                r#type: "gpt-4".to_string(),
                provider: "p1".to_string(),
                params: Default::default(),
            }],
            pipelines: vec![Pipeline {
                name: "pipe1".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![PluginConfig::ModelRouter {
                    models: vec!["m1".to_string()],
                }],
                guards: vec![],
            }],
        };
        assert!(validate_gateway_config(&config).is_ok());
    }

    #[test]
    fn test_invalid_model_provider_ref() {
        let config = GatewayConfig {
            guardrails: None,
            general: None,
            providers: vec![Provider {
                key: "p1".to_string(),
                r#type: ProviderType::OpenAI,
                api_key: "key1".to_string(),
                params: Default::default(),
            }],
            models: vec![ModelConfig {
                key: "m1".to_string(),
                r#type: "gpt-4".to_string(),
                provider: "p2_non_existent".to_string(), // Invalid provider ref
                params: Default::default(),
            }],
            pipelines: vec![],
        };
        let result = validate_gateway_config(&config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("references non-existent provider 'p2_non_existent'"));
    }

    #[test]
    fn test_invalid_pipeline_model_ref() {
        let config = GatewayConfig {
            guardrails: None,
            general: None,
            providers: vec![Provider {
                key: "p1".to_string(),
                r#type: ProviderType::OpenAI,
                api_key: "key1".to_string(),
                params: Default::default(),
            }],
            models: vec![ModelConfig {
                key: "m1".to_string(),
                r#type: "gpt-4".to_string(),
                provider: "p1".to_string(),
                params: Default::default(),
            }],
            pipelines: vec![Pipeline {
                name: "pipe1".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![PluginConfig::ModelRouter {
                    models: vec!["m2_non_existent".to_string()],
                }],
                guards: vec![],
            }],
        };
        let result = validate_gateway_config(&config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("references non-existent model 'm2_non_existent'"));
    }

    #[test]
    fn test_guard_references_non_existent_guardrail_provider() {
        let config = GatewayConfig {
            guardrails: Some(GuardrailsConfig {
                providers: HashMap::from([("gr_p1".to_string(), GrProviderConfig {
                    name: "gr_p1".to_string(),
                    api_base: "http://localhost".to_string(),
                    api_key: "key".to_string(),
                })]),
                guards: vec![Guard {
                    name: "g1".to_string(),
                    provider: "gr_p2_non_existent".to_string(),
                    evaluator_slug: "slug".to_string(),
                    params: Default::default(),
                    mode: GuardMode::PreCall,
                    on_failure: OnFailure::Block,
                    required: true,
                    api_base: None,
                    api_key: None,
                }],
            }),
            general: None,
            providers: vec![],
            models: vec![],
            pipelines: vec![],
        };
        let result = validate_gateway_config(&config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("references non-existent guardrail provider 'gr_p2_non_existent'")));
        assert!(errors.iter().any(|e| e.contains("no api_base configured")));
        assert!(errors.iter().any(|e| e.contains("no api_key configured")));
    }

    #[test]
    fn test_pipeline_references_non_existent_guard() {
        let config = GatewayConfig {
            guardrails: Some(GuardrailsConfig {
                providers: HashMap::from([("gr_p1".to_string(), GrProviderConfig {
                    name: "gr_p1".to_string(),
                    api_base: "http://localhost".to_string(),
                    api_key: "key".to_string(),
                })]),
                guards: vec![Guard {
                    name: "g1".to_string(),
                    provider: "gr_p1".to_string(),
                    evaluator_slug: "slug".to_string(),
                    params: Default::default(),
                    mode: GuardMode::PreCall,
                    on_failure: OnFailure::Block,
                    required: true,
                    api_base: None,
                    api_key: None,
                }],
            }),
            general: None,
            providers: vec![],
            models: vec![],
            pipelines: vec![Pipeline {
                name: "pipe1".to_string(),
                r#type: PipelineType::Chat,
                plugins: vec![],
                guards: vec!["g_non_existent".to_string()],
            }],
        };
        let result = validate_gateway_config(&config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("references non-existent guard 'g_non_existent'"));
    }

    #[test]
    fn test_duplicate_guard_names() {
        let config = GatewayConfig {
            guardrails: Some(GuardrailsConfig {
                providers: HashMap::from([("gr_p1".to_string(), GrProviderConfig {
                    name: "gr_p1".to_string(),
                    api_base: "http://localhost".to_string(),
                    api_key: "key".to_string(),
                })]),
                guards: vec![
                    Guard {
                        name: "g1".to_string(),
                        provider: "gr_p1".to_string(),
                        evaluator_slug: "slug".to_string(),
                        params: Default::default(),
                        mode: GuardMode::PreCall,
                        on_failure: OnFailure::Block,
                        required: true,
                        api_base: None,
                        api_key: None,
                    },
                    Guard {
                        name: "g1".to_string(),
                        provider: "gr_p1".to_string(),
                        evaluator_slug: "slug2".to_string(),
                        params: Default::default(),
                        mode: GuardMode::PostCall,
                        on_failure: OnFailure::Warn,
                        required: true,
                        api_base: None,
                        api_key: None,
                    },
                ],
            }),
            general: None,
            providers: vec![],
            models: vec![],
            pipelines: vec![],
        };
        let result = validate_gateway_config(&config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("Duplicate guard name: 'g1'"));
    }

    #[test]
    fn test_guard_missing_api_base_and_api_key() {
        let config = GatewayConfig {
            guardrails: Some(GuardrailsConfig {
                providers: HashMap::from([("gr_p1".to_string(), GrProviderConfig {
                    name: "gr_p1".to_string(),
                    api_base: "".to_string(),
                    api_key: "".to_string(),
                })]),
                guards: vec![Guard {
                    name: "g1".to_string(),
                    provider: "gr_p1".to_string(),
                    evaluator_slug: "slug".to_string(),
                    params: Default::default(),
                    mode: GuardMode::PreCall,
                    on_failure: OnFailure::Block,
                    required: true,
                    api_base: None,
                    api_key: None,
                }],
            }),
            general: None,
            providers: vec![],
            models: vec![],
            pipelines: vec![],
        };
        let result = validate_gateway_config(&config);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 2);
        assert!(errors[0].contains("no api_base configured"));
        assert!(errors[1].contains("no api_key configured"));
    }

    #[test]
    fn test_guard_inherits_api_base_from_provider() {
        let config = GatewayConfig {
            guardrails: Some(GuardrailsConfig {
                providers: HashMap::from([("gr_p1".to_string(), GrProviderConfig {
                    name: "gr_p1".to_string(),
                    api_base: "http://localhost".to_string(),
                    api_key: "key".to_string(),
                })]),
                guards: vec![Guard {
                    name: "g1".to_string(),
                    provider: "gr_p1".to_string(),
                    evaluator_slug: "slug".to_string(),
                    params: Default::default(),
                    mode: GuardMode::PreCall,
                    on_failure: OnFailure::Block,
                    required: true,
                    api_base: None,
                    api_key: None,
                }],
            }),
            general: None,
            providers: vec![],
            models: vec![],
            pipelines: vec![],
        };
        assert!(validate_gateway_config(&config).is_ok());
    }
}

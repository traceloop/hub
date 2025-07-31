use hub_lib::config;
use std::env;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_config_with_environment_variables() {
    // Set up environment variables
    unsafe {
        env::set_var("TEST_OPENAI_API_KEY", "sk-test-key-123");
        env::set_var("TEST_ANTHROPIC_API_KEY", "sk-ant-test-key-456");
        env::set_var("TEST_AZURE_API_KEY", "azure-test-key-789");
        env::set_var("TEST_RESOURCE_NAME", "test-resource");
        env::set_var("TEST_REGION", "us-west-2");
        env::set_var(
            "TEST_TRACING_ENDPOINT",
            "https://test.traceloop.com/v1/traces",
        );
        env::set_var("TEST_TRACING_API_KEY", "tracing-key-123");
    }

    // Create a temporary config file with environment variable references
    let config_content = r#"
providers:
  - key: openai
    type: openai
    api_key: "${TEST_OPENAI_API_KEY}"
  - key: anthropic
    type: anthropic
    api_key: "${TEST_ANTHROPIC_API_KEY}"
  - key: azure-openai
    type: azure
    api_key: "${TEST_AZURE_API_KEY}"
    resource_name: "${TEST_RESOURCE_NAME}"
    api_version: "2024-06-01"
  - key: bedrock
    type: bedrock
    region: "${TEST_REGION}"
    inference_profile_id: "us"
    AWS_ACCESS_KEY_ID: "test-access-key"
    AWS_SECRET_ACCESS_KEY: "test-secret-key"

models:
  - key: gpt-4o-openai
    type: gpt-4o
    provider: openai
  - key: claude-3-5-sonnet
    type: claude-3-5-sonnet-20241022
    provider: anthropic
  - key: gpt-4o-azure
    type: gpt-4o
    provider: azure-openai
    deployment: "test-deployment"

pipelines:
  - name: default
    type: chat
    plugins:
      - model-router:
          models:
            - gpt-4o-openai
            - claude-3-5-sonnet
      - tracing:
          endpoint: "${TEST_TRACING_ENDPOINT}"
          api_key: "${TEST_TRACING_API_KEY}"
"#;

    // Write to temporary file
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(config_content.as_bytes())
        .expect("Failed to write to temp file");
    let temp_path = temp_file.path().to_str().unwrap();

    // Load the config
    let result = config::load_config(temp_path);
    assert!(result.is_ok(), "Config loading failed: {:?}", result.err());

    let gateway_config = result.unwrap();

    // Verify providers have correct environment variable substitutions
    assert_eq!(gateway_config.providers.len(), 4);

    let openai_provider = gateway_config
        .providers
        .iter()
        .find(|p| p.key == "openai")
        .expect("OpenAI provider not found");
    assert_eq!(openai_provider.api_key, "sk-test-key-123".to_string());

    let anthropic_provider = gateway_config
        .providers
        .iter()
        .find(|p| p.key == "anthropic")
        .expect("Anthropic provider not found");
    assert_eq!(
        anthropic_provider.api_key,
        "sk-ant-test-key-456".to_string()
    );

    let azure_provider = gateway_config
        .providers
        .iter()
        .find(|p| p.key == "azure-openai")
        .expect("Azure provider not found");
    assert_eq!(azure_provider.api_key, "azure-test-key-789".to_string());
    assert_eq!(
        azure_provider.params.get("resource_name"),
        Some(&"test-resource".to_string())
    );

    let bedrock_provider = gateway_config
        .providers
        .iter()
        .find(|p| p.key == "bedrock")
        .expect("Bedrock provider not found");
    assert_eq!(
        bedrock_provider.params.get("region"),
        Some(&"us-west-2".to_string())
    );

    // Verify models are parsed correctly
    assert_eq!(gateway_config.models.len(), 3);

    // Verify pipelines and nested environment variables in plugins
    assert_eq!(gateway_config.pipelines.len(), 1);
    let pipeline = &gateway_config.pipelines[0];
    assert_eq!(pipeline.name, "default");

    // Clean up environment variables
    unsafe {
        env::remove_var("TEST_OPENAI_API_KEY");
        env::remove_var("TEST_ANTHROPIC_API_KEY");
        env::remove_var("TEST_AZURE_API_KEY");
        env::remove_var("TEST_RESOURCE_NAME");
        env::remove_var("TEST_REGION");
        env::remove_var("TEST_TRACING_ENDPOINT");
        env::remove_var("TEST_TRACING_API_KEY");
    }
}

#[test]
fn test_config_with_missing_environment_variable() {
    // Ensure the environment variable is not set
    unsafe {
        env::remove_var("MISSING_TEST_VAR");
    }

    let config_content = r#"
providers:
  - key: openai
    type: openai
    api_key: "${MISSING_TEST_VAR}"

models:
  - key: gpt-4o-openai
    type: gpt-4o
    provider: openai

pipelines:
  - name: default
    type: chat
    plugins:
      - model-router:
          models:
            - gpt-4o-openai
"#;

    // Write to temporary file
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(config_content.as_bytes())
        .expect("Failed to write to temp file");
    let temp_path = temp_file.path().to_str().unwrap();

    // Load the config - should fail
    let result = config::load_config(temp_path);
    assert!(result.is_err(), "Config loading should have failed");

    let error_message = result.unwrap_err().to_string();
    assert!(error_message.contains("Environment variable 'MISSING_TEST_VAR' not found"));
}

#[test]
fn test_config_without_environment_variables() {
    let config_content = r#"
providers:
  - key: openai
    type: openai
    api_key: "sk-static-key-123"

models:
  - key: gpt-4o-openai
    type: gpt-4o
    provider: openai

pipelines:
  - name: default
    type: chat
    plugins:
      - model-router:
          models:
            - gpt-4o-openai
"#;

    // Write to temporary file
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    temp_file
        .write_all(config_content.as_bytes())
        .expect("Failed to write to temp file");
    let temp_path = temp_file.path().to_str().unwrap();

    // Load the config - should succeed
    let result = config::load_config(temp_path);
    assert!(result.is_ok(), "Config loading failed: {:?}", result.err());

    let gateway_config = result.unwrap();
    let openai_provider = gateway_config
        .providers
        .iter()
        .find(|p| p.key == "openai")
        .expect("OpenAI provider not found");
    assert_eq!(openai_provider.api_key, "sk-static-key-123".to_string());
}

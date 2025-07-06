use crate::management::dto::SecretObject;
use anyhow::{anyhow, Result};
use std::env;
use tracing::{debug, warn};

/// Service responsible for resolving SecretObject instances to actual secret values
pub struct SecretResolver;

impl SecretResolver {
    pub fn new() -> Self {
        Self
    }

    /// Resolve a SecretObject to its actual secret value
    pub async fn resolve_secret(&self, secret: &SecretObject) -> Result<String> {
        match secret {
            SecretObject::Literal { value, encrypted } => {
                if encrypted.unwrap_or(false) {
                    // TODO: Implement decryption logic in future phase
                    warn!("Encrypted literal secrets not yet implemented, treating as plaintext");
                }
                debug!("Resolving literal secret");
                Ok(value.clone())
            }

            SecretObject::Environment { variable_name } => {
                debug!("Resolving environment variable: {}", variable_name);
                env::var(variable_name)
                    .map_err(|_| anyhow!("Environment variable '{}' not found", variable_name))
            }

            SecretObject::Kubernetes {
                secret_name,
                key,
                namespace,
            } => {
                debug!(
                    "Resolving Kubernetes secret: {}/{} in namespace {:?}",
                    secret_name, key, namespace
                );
                // TODO: Implement Kubernetes secret resolution in future phase
                Err(anyhow!(
                    "Kubernetes secret resolution not yet implemented for secret '{}' key '{}'",
                    secret_name,
                    key
                ))
            }
        }
    }

    /// Resolve an optional SecretObject
    pub async fn resolve_optional_secret(
        &self,
        secret: &Option<SecretObject>,
    ) -> Result<Option<String>> {
        match secret {
            Some(secret_obj) => Ok(Some(self.resolve_secret(secret_obj).await?)),
            None => Ok(None),
        }
    }
}

impl Default for SecretResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_resolve_literal_secret() {
        let resolver = SecretResolver::new();
        let secret = SecretObject::literal("test-api-key".to_string());

        let result = resolver.resolve_secret(&secret).await.unwrap();
        assert_eq!(result, "test-api-key");
    }

    #[tokio::test]
    async fn test_resolve_environment_secret() {
        let resolver = SecretResolver::new();
        let test_var = "TEST_SECRET_VAR";
        let test_value = "test-secret-value";

        // Set environment variable for test
        env::set_var(test_var, test_value);

        let secret = SecretObject::environment(test_var.to_string());
        let result = resolver.resolve_secret(&secret).await.unwrap();

        assert_eq!(result, test_value);

        // Clean up
        env::remove_var(test_var);
    }

    #[tokio::test]
    async fn test_resolve_missing_environment_secret() {
        let resolver = SecretResolver::new();
        let secret = SecretObject::environment("NON_EXISTENT_VAR".to_string());

        let result = resolver.resolve_secret(&secret).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Environment variable 'NON_EXISTENT_VAR' not found"));
    }

    #[tokio::test]
    async fn test_resolve_kubernetes_secret_not_implemented() {
        let resolver = SecretResolver::new();
        let secret = SecretObject::kubernetes(
            "my-secret".to_string(),
            "api-key".to_string(),
            Some("default".to_string()),
        );

        let result = resolver.resolve_secret(&secret).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Kubernetes secret resolution not yet implemented"));
    }

    #[tokio::test]
    async fn test_resolve_optional_secret() {
        let resolver = SecretResolver::new();

        // Test with Some
        let secret = Some(SecretObject::literal("test-value".to_string()));
        let result = resolver.resolve_optional_secret(&secret).await.unwrap();
        assert_eq!(result, Some("test-value".to_string()));

        // Test with None
        let result = resolver.resolve_optional_secret(&None).await.unwrap();
        assert_eq!(result, None);
    }
}

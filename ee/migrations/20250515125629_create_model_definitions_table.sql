-- Add migration script here

-- Up
CREATE TABLE hub_llmgateway_ee_model_definitions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key TEXT NOT NULL UNIQUE,                 -- e.g., "gpt-4o-openai", "my-custom-claude"
    model_type TEXT NOT NULL,             -- e.g., "gpt-4o", "claude-3-opus-20240229"
    provider_id UUID NOT NULL,
    config_details JSONB,                     -- Provider-specific model configurations
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_provider
        FOREIGN KEY(provider_id)
        REFERENCES hub_llmgateway_ee_providers(id)
        ON DELETE CASCADE -- If a provider is deleted, its associated model definitions are also deleted.
);

-- Index for faster lookups by key
CREATE UNIQUE INDEX idx_model_definitions_key ON hub_llmgateway_ee_model_definitions(key);

-- Index for faster lookups by provider_id
CREATE INDEX idx_model_definitions_provider_id ON hub_llmgateway_ee_model_definitions(provider_id);

-- Trigger to automatically update updated_at timestamp
-- Assumes update_modified_column function is created by a previous migration (e.g., for providers table)
CREATE TRIGGER set_timestamp_model_definitions
BEFORE UPDATE ON hub_llmgateway_ee_model_definitions
FOR EACH ROW
EXECUTE PROCEDURE update_modified_column();

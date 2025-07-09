-- Migration: <timestamp>_initial_schema.sql
CREATE EXTENSION IF NOT EXISTS "pgcrypto";


CREATE TABLE IF NOT EXISTS hub_llmgateway_providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,       -- User-defined unique name for this provider config instance
    provider_type VARCHAR(50) NOT NULL,     -- e.g., 'openai', 'azure', 'bedrock' (matches ProviderType enum variants)
    config_details JSONB NOT NULL,          -- Stores specific config like API keys, region, resource_name
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Optional: Add an index on provider_type for faster lookups if you often query by type
CREATE INDEX IF NOT EXISTS idx_hub_llmgateway_providers_provider_type ON hub_llmgateway_providers(provider_type);

-- Optional: Trigger to automatically update updated_at timestamp
CREATE OR REPLACE FUNCTION update_modified_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_hub_llmgateway_providers_modtime
    BEFORE UPDATE ON hub_llmgateway_providers
    FOR EACH ROW
    EXECUTE FUNCTION update_modified_column();

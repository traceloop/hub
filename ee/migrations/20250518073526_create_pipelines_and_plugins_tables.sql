-- Add migration script here

-- Table for Pipelines
CREATE TABLE hub_llmgateway_ee_pipelines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) UNIQUE NOT NULL,
    pipeline_type VARCHAR(100) NOT NULL,
    description TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Table for Pipeline Plugin Configurations
CREATE TABLE hub_llmgateway_ee_pipeline_plugin_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    pipeline_id UUID NOT NULL REFERENCES hub_llmgateway_ee_pipelines(id) ON DELETE CASCADE,
    plugin_name VARCHAR(100) NOT NULL,
    config_data JSONB NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    order_in_pipeline INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_pipeline_plugin UNIQUE (pipeline_id, plugin_name)
);

-- Create indexes for faster lookups
CREATE INDEX idx_pipeline_name ON hub_llmgateway_ee_pipelines(name);
CREATE INDEX idx_pipeline_plugin_pipeline_id ON hub_llmgateway_ee_pipeline_plugin_configs(pipeline_id);
CREATE INDEX idx_pipeline_plugin_name ON hub_llmgateway_ee_pipeline_plugin_configs(plugin_name);

-- Trigger to update 'updated_at' timestamp on row update for pipelines
CREATE OR REPLACE FUNCTION update_modified_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_pipelines_updated_at
BEFORE UPDATE ON hub_llmgateway_ee_pipelines
FOR EACH ROW
EXECUTE FUNCTION update_modified_column();

-- Trigger to update 'updated_at' timestamp on row update for pipeline_plugin_configs
CREATE TRIGGER update_pipeline_plugin_configs_updated_at
BEFORE UPDATE ON hub_llmgateway_ee_pipeline_plugin_configs
FOR EACH ROW
EXECUTE FUNCTION update_modified_column();

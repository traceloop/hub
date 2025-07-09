#!/bin/bash

# Traceloop Hub Gateway - Sample Configuration Creator
# This script creates sample configuration via the EE Management API

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
GATEWAY_URL="http://localhost:3000"
MANAGEMENT_URL="http://localhost:8080"
API_BASE="${MANAGEMENT_URL}/api/v1/management"

echo -e "${BLUE}🔧 Creating Sample Configuration for EE Gateway${NC}"
echo "=============================================="

# Check if gateway is running
echo -e "${BLUE}1. Checking if gateway is running...${NC}"
if ! curl -s "${MANAGEMENT_URL}/health" > /dev/null; then
    echo -e "${RED}❌ Management API is not running at ${MANAGEMENT_URL}${NC}"
    echo -e "${YELLOW}Please start the gateway first:${NC}"
    echo -e "   ${BLUE}cargo run --features db_based_config${NC}"
    exit 1
fi
echo -e "${GREEN}   ✅ Gateway is running${NC}"

# Create OpenAI Provider
echo -e "${BLUE}2. Creating OpenAI provider...${NC}"
OPENAI_PROVIDER_RESPONSE=$(curl -s -X POST "${API_BASE}/providers" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "OpenAI Main",
    "provider_type": "openai",
    "config": {
      "api_key": {
        "type": "environment",
        "variable_name": "OPENAI_API_KEY"
      },
      "organization_id": null
    },
    "enabled": true
  }')

OPENAI_PROVIDER_ID=$(echo "$OPENAI_PROVIDER_RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$OPENAI_PROVIDER_ID" ]; then
    echo -e "${RED}❌ Failed to create OpenAI provider${NC}"
    echo "Response: $OPENAI_PROVIDER_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ OpenAI provider created (ID: $OPENAI_PROVIDER_ID)${NC}"

# Create Azure Provider
echo -e "${BLUE}3. Creating Azure provider...${NC}"
AZURE_PROVIDER_RESPONSE=$(curl -s -X POST "${API_BASE}/providers" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Azure OpenAI",
    "provider_type": "azure",
    "config": {
        "api_key": {
          "type": "environment",
          "variable_name": "AZURE_OPENAI_API_KEY"
        },
        "resource_name": "<your azure resource>",
        "api_version": "<your azure api version>"
    },
    "enabled": true
  }')

AZURE_PROVIDER_ID=$(echo "$AZURE_PROVIDER_RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
if [ -z "$AZURE_PROVIDER_ID" ]; then
    echo -e "${RED}❌ Failed to create Azure provider${NC}"
    echo "Response: $AZURE_PROVIDER_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ Azure provider created (ID: $AZURE_PROVIDER_ID)${NC}"

# Create Model Definitions
echo -e "${BLUE}4. Creating model definitions...${NC}"

# GPT-4 model
GPT4_1_MODEL_RESPONSE=$(curl -s -X POST "${API_BASE}/model-definitions" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": \"gpt-4.1\",
    \"model_type\": \"gpt-4.1\",
    \"provider_id\": \"$OPENAI_PROVIDER_ID\",
    \"config_details\": {},
    \"enabled\": true
  }")

if ! echo "$GPT4_1_MODEL_RESPONSE" | grep -q '"key":"gpt-4.1"'; then
    echo -e "${RED}❌ Failed to create GPT-4.1 model${NC}"
    echo "Response: $GPT4_1_MODEL_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ GPT-4.1 model created${NC}"

# GPT-3.5-turbo model
GPT35_MODEL_RESPONSE=$(curl -s -X POST "${API_BASE}/model-definitions" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": \"gpt-3.5-turbo\",
    \"model_type\": \"gpt-3.5-turbo\",
    \"provider_id\": \"$OPENAI_PROVIDER_ID\",
    \"config_details\": {},
    \"enabled\": true
  }")

if ! echo "$GPT35_MODEL_RESPONSE" | grep -q '"key":"gpt-3.5-turbo"'; then
    echo -e "${RED}❌ Failed to create GPT-3.5-turbo model${NC}"
    echo "Response: $GPT35_MODEL_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ GPT-3.5-turbo model created${NC}"

# Azure GPT-4 model
AZURE_GPT4_MODEL_RESPONSE=$(curl -s -X POST "${API_BASE}/model-definitions" \
  -H "Content-Type: application/json" \
  -d "{
    \"key\": \"gpt-4o-azure\",
    \"model_type\": \"gpt-4o\",
    \"provider_id\": \"$AZURE_PROVIDER_ID\",
    \"config_details\": { \"deployment\": \"evaluation-gpt-4o\" },
    \"enabled\": true
  }")

if ! echo "$AZURE_GPT4_MODEL_RESPONSE" | grep -q '"key":"gpt-4o-azure"'; then
    echo -e "${RED}❌ Failed to create Azure GPT-4 model${NC}"
    echo "Response: $AZURE_GPT4_MODEL_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ Azure GPT-4 model created${NC}"

# Create Pipelines
echo -e "${BLUE}5. Creating pipelines...${NC}"

# Chat pipeline with multiple models
CHAT_PIPELINE_RESPONSE=$(curl -s -X POST "${API_BASE}/pipelines" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "default",
    "pipeline_type": "Chat",
    "plugins": [
      {
        "plugin_type": "logging",
        "config_data": {
          "level": "debug"
        }
      },
      {
        "plugin_type": "tracing",
        "config_data": {
          "endpoint": "https://api.traceloop.com/v1/traces",
          "api_key": {
            "type": "environment",
            "variable_name": "TRACELOOP_API_KEY"
          }
        }
      },
      {
        "plugin_type": "model-router",
        "config_data": {
          "models": [
            {
              "key": "gpt-4o-azure",
              "priority": 0
            },
            {
              "key": "gpt-4.1",
              "priority": 1
            },
            {
              "key": "gpt-3.5-turbo",
              "priority": 2
            }
          ]
        }
      }
    ],
    "enabled": true
  }')

if ! echo "$CHAT_PIPELINE_RESPONSE" | grep -q '"name":"default"'; then
    echo -e "${RED}❌ Failed to create chat pipeline${NC}"
    echo "Response: $CHAT_PIPELINE_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ Chat pipeline created${NC}"

# Simple pipeline with single model
SIMPLE_PIPELINE_RESPONSE=$(curl -s -X POST "${API_BASE}/pipelines" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "simple-pipeline",
    "pipeline_type": "Chat",
    "plugins": [
      {
        "plugin_type": "logging",
        "config_data": {
          "level": "info"
        }
      },
      {
        "plugin_type": "tracing",
        "config_data": {
          "endpoint": "https://api.traceloop.com/v1/traces",
          "api_key": {
            "type": "environment",
            "variable_name": "TRACELOOP_API_KEY"
          }
        }
      },
      {
        "plugin_type": "model-router",
        "config_data": {
          "models": [
            {
              "key": "gpt-3.5-turbo",
              "priority": 0
            }
          ]
        }
      }
    ],
    "enabled": true
  }')

if ! echo "$SIMPLE_PIPELINE_RESPONSE" | grep -q '"name":"simple-pipeline"'; then
    echo -e "${RED}❌ Failed to create simple pipeline${NC}"
    echo "Response: $SIMPLE_PIPELINE_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ Simple pipeline created${NC}"

echo ""
echo -e "${GREEN}🎉 Sample configuration created successfully!${NC}"
echo ""
echo -e "${YELLOW}Configuration Summary:${NC}"
echo -e "${BLUE}Providers:${NC}"
echo "  • OpenAI Main (ID: $OPENAI_PROVIDER_ID)"
echo "  • Azure OpenAI (ID: $AZURE_PROVIDER_ID)"
echo ""
echo -e "${BLUE}Models:${NC}"
echo "  • gpt-4.1 (OpenAI)"
echo "  • gpt-3.5-turbo (OpenAI)"
echo "  • gpt-4o-azure (Azure)"
echo ""
echo -e "${BLUE}Pipelines:${NC}"
echo "  • default (multi-model routing with logging and tracing)"
echo "  • simple-pipeline (single model with logging and without tracing)"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "1. Set up environment variables for tracing:"
echo -e "   ${BLUE}export TRACELOOP_API_KEY=\"your-traceloop-api-key\"${NC}"
echo ""
echo "2. Update API keys in the providers using SecretObject format:"
echo ""
echo -e "${BLUE}   # Option A: Literal secret (for testing)${NC}"
echo -e "   ${BLUE}curl -X PUT ${API_BASE}/providers/$OPENAI_PROVIDER_ID \\${NC}"
echo -e "   ${BLUE}     -H \"Content-Type: application/json\" \\${NC}"
echo -e "   ${BLUE}     -d '{\"config\": {\"api_key\": {\"type\": \"literal\", \"value\": \"your-real-openai-key\"}}}'${NC}"
echo ""
echo -e "${BLUE}   # Option B: Environment variable (recommended for production)${NC}"
echo -e "   ${BLUE}curl -X PUT ${API_BASE}/providers/$OPENAI_PROVIDER_ID \\${NC}"
echo -e "   ${BLUE}     -H \"Content-Type: application/json\" \\${NC}"
echo -e "   ${BLUE}     -d '{\"config\": {\"api_key\": {\"type\": \"environment\", \"variable_name\": \"OPENAI_API_KEY\"}}}'${NC}"
echo ""
echo -e "${BLUE}   # Option C: Kubernetes secret (for K8s deployments)${NC}"
echo -e "   ${BLUE}curl -X PUT ${API_BASE}/providers/$OPENAI_PROVIDER_ID \\${NC}"
echo -e "   ${BLUE}     -H \"Content-Type: application/json\" \\${NC}"
echo -e "   ${BLUE}     -d '{\"config\": {\"api_key\": {\"type\": \"kubernetes\", \"secret_name\": \"openai-creds\", \"key\": \"api-key\"}}}'${NC}"
echo ""
echo "3. Test the configuration:"
echo -e "   ${BLUE}curl ${API_BASE}/providers${NC}"
echo -e "   ${BLUE}curl ${API_BASE}/model-definitions${NC}"
echo -e "   ${BLUE}curl ${API_BASE}/pipelines${NC}"
echo ""
echo "4. The gateway will automatically pick up the configuration within 30 seconds!"
echo ""
echo -e "${GREEN}Happy testing! 🚀${NC}" 
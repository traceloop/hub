#!/bin/bash

# Traceloop Hub Gateway - Add GPT-4.1 Mini Model
# This script adds a new GPT-4.1 mini model to the existing OpenAI provider
# and updates the default pipeline to include it

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

echo -e "${BLUE}🚀 Adding GPT-4.1 Mini Model to EE Gateway${NC}"
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

# Get existing OpenAI provider
echo -e "${BLUE}2. Finding OpenAI provider...${NC}"
PROVIDERS_RESPONSE=$(curl -s "${API_BASE}/providers")
OPENAI_PROVIDER_ID=$(echo "$PROVIDERS_RESPONSE" | grep -o '"id":"[^"]*","name":"OpenAI Main"' | cut -d'"' -f4)

if [ -z "$OPENAI_PROVIDER_ID" ]; then
    echo -e "${RED}❌ OpenAI provider not found${NC}"
    echo -e "${YELLOW}Please run create-sample-config.sh first to create the base configuration${NC}"
    exit 1
fi
echo -e "${GREEN}   ✅ OpenAI provider found (ID: $OPENAI_PROVIDER_ID)${NC}"

# Check if GPT-4.1 mini model already exists
echo -e "${BLUE}3. Checking if GPT-4.1 mini model already exists...${NC}"
MODELS_RESPONSE=$(curl -s "${API_BASE}/model-definitions")
if echo "$MODELS_RESPONSE" | grep -q '"key":"gpt-4.1-mini"'; then
    echo -e "${YELLOW}   ⚠️  GPT-4.1 mini model already exists, skipping creation${NC}"
    GPT4_MINI_EXISTS=true
else
    GPT4_MINI_EXISTS=false
fi

# Create GPT-4.1 mini model if it doesn't exist
if [ "$GPT4_MINI_EXISTS" = false ]; then
    echo -e "${BLUE}4. Creating GPT-4.1 mini model...${NC}"
    GPT4_MINI_MODEL_RESPONSE=$(curl -s -X POST "${API_BASE}/model-definitions" \
      -H "Content-Type: application/json" \
      -d "{
        \"key\": \"gpt-4.1-mini\",
        \"model_type\": \"gpt-4.1-mini\",
        \"provider_id\": \"$OPENAI_PROVIDER_ID\",
        \"config_details\": {},
        \"enabled\": true
      }")

    if ! echo "$GPT4_MINI_MODEL_RESPONSE" | grep -q '"key":"gpt-4.1-mini"'; then
        echo -e "${RED}❌ Failed to create GPT-4.1 mini model${NC}"
        echo "Response: $GPT4_MINI_MODEL_RESPONSE"
        exit 1
    fi
    echo -e "${GREEN}   ✅ GPT-4.1 mini model created${NC}"
else
    echo -e "${GREEN}   ✅ GPT-4.1 mini model already exists${NC}"
fi

# Get the default pipeline
echo -e "${BLUE}5. Finding default pipeline...${NC}"
PIPELINES_RESPONSE=$(curl -s "${API_BASE}/pipelines")
DEFAULT_PIPELINE_ID=$(echo "$PIPELINES_RESPONSE" | grep -o '"id":"[^"]*","name":"default"' | cut -d'"' -f4)

if [ -z "$DEFAULT_PIPELINE_ID" ]; then
    echo -e "${RED}❌ Default pipeline not found${NC}"
    echo -e "${YELLOW}Please run create-sample-config.sh first to create the base configuration${NC}"
    exit 1
fi
echo -e "${GREEN}   ✅ Default pipeline found (ID: $DEFAULT_PIPELINE_ID)${NC}"

# Get current pipeline configuration
echo -e "${BLUE}6. Getting current pipeline configuration...${NC}"
CURRENT_PIPELINE=$(curl -s "${API_BASE}/pipelines/${DEFAULT_PIPELINE_ID}")

# Check if GPT-4.1 mini is already in the pipeline
if echo "$CURRENT_PIPELINE" | grep -q '"key":"gpt-4.1-mini"'; then
    echo -e "${YELLOW}   ⚠️  GPT-4.1 mini is already in the default pipeline${NC}"
    echo -e "${GREEN}🎉 Configuration is already up to date!${NC}"
    exit 0
fi

# Update pipeline to include GPT-4.1 mini
echo -e "${BLUE}7. Updating default pipeline to include GPT-4.1 mini...${NC}"
UPDATE_PIPELINE_RESPONSE=$(curl -s -X PUT "${API_BASE}/pipelines/${DEFAULT_PIPELINE_ID}" \
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
              "key": "gpt-4.1-mini",
              "priority": 2
            },
            {
              "key": "gpt-3.5-turbo",
              "priority": 3
            }
          ]
        }
      }
    ],
    "enabled": true
  }')

if ! echo "$UPDATE_PIPELINE_RESPONSE" | grep -q '"name":"default"'; then
    echo -e "${RED}❌ Failed to update default pipeline${NC}"
    echo "Response: $UPDATE_PIPELINE_RESPONSE"
    exit 1
fi
echo -e "${GREEN}   ✅ Default pipeline updated with GPT-4.1 mini${NC}"

echo ""
echo -e "${GREEN}🎉 GPT-4.1 Mini model added successfully!${NC}"
echo ""
echo -e "${YELLOW}Configuration Summary:${NC}"
echo -e "${BLUE}New Model Added:${NC}"
echo "  • gpt-4.1-mini (OpenAI) - Priority 2 in default pipeline"
echo ""
echo -e "${BLUE}Updated Pipeline Routing Order:${NC}"
echo "  1. gpt-4o-azure (Priority 0) - Primary"
echo "  2. gpt-4.1 (Priority 1) - Secondary"
echo "  3. gpt-4.1-mini (Priority 2) - Tertiary (NEW)"
echo "  4. gpt-3.5-turbo (Priority 3) - Fallback"
echo ""
echo -e "${YELLOW}Next Steps:${NC}"
echo "1. The gateway will automatically pick up the new configuration within 30 seconds"
echo ""
echo "2. Test the new model:"
echo -e "   ${BLUE}curl -X POST ${GATEWAY_URL}/v1/chat/completions \\${NC}"
echo -e "   ${BLUE}     -H \"Content-Type: application/json\" \\${NC}"
echo -e "   ${BLUE}     -d '{\"model\": \"gpt-4.1-mini\", \"messages\": [{\"role\": \"user\", \"content\": \"Hello!\"}]}'${NC}"
echo ""
echo "3. View updated configuration:"
echo -e "   ${BLUE}curl ${API_BASE}/model-definitions${NC}"
echo -e "   ${BLUE}curl ${API_BASE}/pipelines/${DEFAULT_PIPELINE_ID}${NC}"
echo ""
echo -e "${GREEN}Happy testing with GPT-4.1 Mini! 🚀${NC}" 
#!/bin/bash

# Traceloop Hub Gateway - Sample Configuration Deleter
# This script deletes sample configuration via the EE Management API

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
GATEWAY_URL="http://localhost:3100"
API_BASE="${GATEWAY_URL}/api/v1/ee"

echo -e "${BLUE}🔧 Deleting Sample Configuration for EE Gateway${NC}"
echo "=============================================="

# Check if gateway is running
echo -e "${BLUE}1. Checking if gateway is running...${NC}"
if ! curl -s "${API_BASE}/health" > /dev/null; then
    echo -e "${RED}❌ Gateway is not running at ${GATEWAY_URL}${NC}"
    echo -e "${YELLOW}Please start the gateway first:${NC}"
    echo -e "   ${BLUE}cargo run --features ee_feature${NC}"
    exit 1
fi
echo -e "${GREEN}   ✅ Gateway is running${NC}"

# Get ALL Pipelines
echo -e "${BLUE}2. Getting all pipelines...${NC}"
PIPELINES_RESPONSE=$(curl -s "${API_BASE}/pipelines")
echo "$PIPELINES_RESPONSE"

# Delete ALL Pipelines
PIPELINES_IDS=$(echo "$PIPELINES_RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
for PIPELINE_ID in $PIPELINES_IDS; do
    echo -e "${BLUE}3. Deleting pipeline $PIPELINE_ID...${NC}"
    PIPELINE_RESPONSE=$(curl -s -X DELETE "${API_BASE}/pipelines/$PIPELINE_ID")
    echo "$PIPELINE_RESPONSE"
done

# Get ALL Model Definitions
echo -e "${BLUE}4. Getting all model definitions...${NC}"
MODEL_DEFINITIONS_RESPONSE=$(curl -s "${API_BASE}/model-definitions")
echo "$MODEL_DEFINITIONS_RESPONSE"

# Delete ALL Model Definitions
MODEL_DEFINITIONS_IDS=$(echo "$MODEL_DEFINITIONS_RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
for MODEL_DEFINITION_ID in $MODEL_DEFINITIONS_IDS; do
    echo -e "${BLUE}5. Deleting model definition $MODEL_DEFINITION_ID...${NC}"
    MODEL_DEFINITION_RESPONSE=$(curl -s -X DELETE "${API_BASE}/model-definitions/$MODEL_DEFINITION_ID")
    echo "$MODEL_DEFINITION_RESPONSE"
done

# Get ALL Providers
echo -e "${BLUE}6. Getting all providers...${NC}"
PROVIDERS_RESPONSE=$(curl -s "${API_BASE}/providers")
echo "$PROVIDERS_RESPONSE"

# Delete ALL Providers
PROVIDERS_IDS=$(echo "$PROVIDERS_RESPONSE" | grep -o '"id":"[^"]*"' | cut -d'"' -f4)
for PROVIDER_ID in $PROVIDERS_IDS; do
    echo -e "${BLUE}7. Deleting provider $PROVIDER_ID...${NC}"
    PROVIDER_RESPONSE=$(curl -s -X DELETE "${API_BASE}/providers/$PROVIDER_ID")
    echo "$PROVIDER_RESPONSE"
done

#verify that all pipelines, model definitions, and providers are deleted
echo -e "${BLUE}8. Verifying that all pipelines, model definitions, and providers are deleted...${NC}"
PIPELINES_RESPONSE=$(curl -s "${API_BASE}/pipelines")
echo "$PIPELINES_RESPONSE"
MODEL_DEFINITIONS_RESPONSE=$(curl -s "${API_BASE}/model-definitions")
echo "$MODEL_DEFINITIONS_RESPONSE"
PROVIDERS_RESPONSE=$(curl -s "${API_BASE}/providers")
echo "$PROVIDERS_RESPONSE"

echo -e "${GREEN}All pipelines, model definitions, and providers have been deleted.${NC}"



echo -e "${GREEN}Happy testing! 🚀${NC}" 
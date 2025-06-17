#!/bin/bash

# Traceloop Hub Gateway - Enterprise Edition Setup Script
# This script automates the setup of EE mode with PostgreSQL

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
DB_NAME="hub_gateway"
DB_USER="hub_user"
DB_PASSWORD="hub_password"
DB_PORT="5432"
CONTAINER_NAME="hub-postgres"
GATEWAY_PORT="3000"

echo -e "${BLUE}🚀 Traceloop Hub Gateway - Enterprise Edition Setup${NC}"
echo "=================================================="

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo -e "${RED}❌ Docker is required but not installed. Please install Docker first.${NC}"
    exit 1
fi

# Check if sqlx-cli is installed
if ! command -v sqlx &> /dev/null; then
    echo -e "${YELLOW}⚠️  sqlx-cli not found. Installing...${NC}"
    cargo install sqlx-cli --no-default-features --features postgres
fi

# Check if Rust/Cargo is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}❌ Rust/Cargo is required but not installed. Please install Rust first.${NC}"
    exit 1
fi

echo -e "${BLUE}1. Setting up PostgreSQL database...${NC}"

# Stop and remove existing container if it exists
if docker ps -a --format 'table {{.Names}}' | grep -q "^${CONTAINER_NAME}$"; then
    echo "   Stopping existing container..."
    docker stop ${CONTAINER_NAME} >/dev/null 2>&1 || true
    docker rm ${CONTAINER_NAME} >/dev/null 2>&1 || true
fi

# Start PostgreSQL container
echo "   Starting PostgreSQL container..."
docker run --name ${CONTAINER_NAME} \
  -e POSTGRES_DB=${DB_NAME} \
  -e POSTGRES_USER=${DB_USER} \
  -e POSTGRES_PASSWORD=${DB_PASSWORD} \
  -p ${DB_PORT}:5432 \
  -d postgres:15 >/dev/null

# Wait for PostgreSQL to be ready
echo "   Waiting for PostgreSQL to be ready..."
for i in {1..30}; do
    if docker exec ${CONTAINER_NAME} pg_isready -U ${DB_USER} -d ${DB_NAME} >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

if ! docker exec ${CONTAINER_NAME} pg_isready -U ${DB_USER} -d ${DB_NAME} >/dev/null 2>&1; then
    echo -e "${RED}❌ PostgreSQL failed to start within 30 seconds${NC}"
    exit 1
fi

echo -e "${GREEN}   ✅ PostgreSQL is ready${NC}"

echo -e "${BLUE}2. Running database migrations...${NC}"

# Set database URL
export DATABASE_URL="postgresql://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}"

# Navigate to ee directory and run migrations
cd ee
sqlx migrate run
cd ..

echo -e "${GREEN}   ✅ Database migrations completed${NC}"

echo -e "${BLUE}3. Building gateway with EE features...${NC}"
cargo build --features ee_feature

echo -e "${GREEN}   ✅ Build completed${NC}"

echo -e "${BLUE}4. Creating environment configuration...${NC}"

# Create .env file
cat > .env << EOF
# Traceloop Hub Gateway - Enterprise Edition Configuration
DATABASE_URL=postgresql://${DB_USER}:${DB_PASSWORD}@localhost:${DB_PORT}/${DB_NAME}
DB_POLL_INTERVAL_SECONDS=30
PORT=${GATEWAY_PORT}
RUST_LOG=info
EOF

echo -e "${GREEN}   ✅ Environment configuration created (.env file)${NC}"

echo ""
echo -e "${GREEN}🎉 Setup completed successfully!${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Start the gateway:"
echo -e "   ${BLUE}cargo run --features ee_feature${NC}"
echo ""
echo "2. Verify it's running:"
echo -e "   ${BLUE}curl http://localhost:${GATEWAY_PORT}/ee/api/v1/health${NC}"
echo ""
echo "3. Create initial configuration using the Management API:"
echo -e "   ${BLUE}# Create a provider${NC}"
echo -e "   ${BLUE}curl -X POST http://localhost:${GATEWAY_PORT}/ee/api/v1/providers \\${NC}"
echo -e "   ${BLUE}     -H \"Content-Type: application/json\" \\${NC}"
echo -e "   ${BLUE}     -d '{${NC}"
echo -e "   ${BLUE}       \"name\": \"OpenAI Main\",${NC}"
echo -e "   ${BLUE}       \"provider_type\": \"OpenAI\",${NC}"
echo -e "   ${BLUE}       \"config\": {${NC}"
echo -e "   ${BLUE}         \"OpenAI\": {${NC}"
echo -e "   ${BLUE}           \"api_key\": \"your-openai-api-key\",${NC}"
echo -e "   ${BLUE}           \"organization_id\": null${NC}"
echo -e "   ${BLUE}         }${NC}"
echo -e "   ${BLUE}       },${NC}"
echo -e "   ${BLUE}       \"enabled\": true${NC}"
echo -e "   ${BLUE}     }'${NC}"
echo ""
echo -e "${YELLOW}Useful commands:${NC}"
echo -e "   ${BLUE}# Stop the database${NC}"
echo -e "   ${BLUE}docker stop ${CONTAINER_NAME}${NC}"
echo ""
echo -e "   ${BLUE}# Start the database (after stopping)${NC}"
echo -e "   ${BLUE}docker start ${CONTAINER_NAME}${NC}"
echo ""
echo -e "   ${BLUE}# View database logs${NC}"
echo -e "   ${BLUE}docker logs ${CONTAINER_NAME}${NC}"
echo ""
echo -e "   ${BLUE}# Connect to database${NC}"
echo -e "   ${BLUE}docker exec -it ${CONTAINER_NAME} psql -U ${DB_USER} -d ${DB_NAME}${NC}"
echo ""
echo -e "${YELLOW}Documentation:${NC}"
echo -e "   ${BLUE}docs/EE_SETUP.md${NC} - Detailed setup guide"
echo -e "   ${BLUE}README.md${NC} - Complete documentation"
echo ""
echo -e "${GREEN}Happy coding! 🚀${NC}" 
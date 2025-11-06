#!/usr/bin/env bash

#
# StarRocks Admin - Frontend Build Script
# Builds the Angular frontend and outputs to frontend/dist/
# Backend will directly embed from frontend/dist/ (no copy needed)
#

set -e

# Get project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="$PROJECT_ROOT/frontend"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Building StarRocks Admin Frontend${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

echo -e "${YELLOW}[1/2]${NC} Installing frontend dependencies..."
cd "$FRONTEND_DIR"
npm install

echo -e "${YELLOW}[2/2]${NC} Building Angular frontend (production mode)..."
# SIMPLIFIED: No need to configure BASE_HREF anymore!
# The backend auto-injects <base href> based on X-Forwarded-Prefix header
# This makes the same build work for both root (/) and sub-path (/xxx) deployments
echo "  Building with auto-detection mode (works for any deployment path)"
npm run build -- --configuration production

echo ""
echo -e "${GREEN}✓ Frontend build complete!${NC}"
echo -e "  Output: $FRONTEND_DIR/dist/"
echo -e "  Note: Backend will embed directly from this directory"

#!/usr/bin/env bash

#
# Verify that the built frontend has the correct base href
#

set -e

# Get project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INDEX_HTML="$PROJECT_ROOT/frontend/dist/index.html"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "Verifying base href in built frontend..."
echo ""

if [ ! -f "$INDEX_HTML" ]; then
    echo -e "${RED}✗ Error: $INDEX_HTML not found${NC}"
    echo "  Please build the frontend first:"
    echo "    cd $PROJECT_ROOT && ./build/build-frontend.sh"
    exit 1
fi

# Check for base href
if grep -q '<base href="./">' "$INDEX_HTML"; then
    echo -e "${GREEN}✓ Base href is correctly set to './'${NC}"
    echo ""
    echo "Found in index.html:"
    grep '<base href' "$INDEX_HTML" | head -1
    exit 0
elif grep -q '<base href="/">' "$INDEX_HTML"; then
    echo -e "${RED}✗ Base href is still set to '/' (absolute path)${NC}"
    echo ""
    echo "Found in index.html:"
    grep '<base href' "$INDEX_HTML" | head -1
    echo ""
    echo "This will cause issues with sub-path deployments."
    echo "Please rebuild the frontend with: ./build/build-frontend.sh"
    exit 1
else
    echo -e "${YELLOW}⚠ Warning: No base href found in index.html${NC}"
    echo "  This might be okay if Angular handles it differently,"
    echo "  but please verify the build output."
    exit 0
fi


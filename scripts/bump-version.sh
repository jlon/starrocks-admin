#!/usr/bin/env bash

#
# StarRocks Admin - Version Bump Script
# Usage: ./scripts/bump-version.sh <version>
# Example: ./scripts/bump-version.sh 1.2.3
#

set -e

VERSION=$1

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Show usage
if [ -z "$VERSION" ]; then
  echo -e "${RED}Error: Version number is required${NC}"
  echo ""
  echo "Usage: $0 <version>"
  echo "Example: $0 1.2.3"
  echo ""
  exit 1
fi

# Validate version format (semantic versioning)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo -e "${RED}Error: Invalid version format${NC}"
  echo "Version must follow semantic versioning: X.Y.Z (e.g., 1.2.3)"
  exit 1
fi

echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Bumping version to $VERSION${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Get project root
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Files to update
CARGO_TOML="$PROJECT_ROOT/backend/Cargo.toml"
PACKAGE_JSON="$PROJECT_ROOT/frontend/package.json"
CHART_YAML="$PROJECT_ROOT/deploy/chart/Chart.yaml"

# Check if files exist
for file in "$CARGO_TOML" "$PACKAGE_JSON" "$CHART_YAML"; do
  if [ ! -f "$file" ]; then
    echo -e "${RED}Error: File not found: $file${NC}"
    exit 1
  fi
done

# Show current versions
echo -e "${YELLOW}Current versions:${NC}"
echo "  Cargo.toml:   $(grep '^version' "$CARGO_TOML" | head -1 | cut -d'"' -f2)"
echo "  package.json: $(grep '"version"' "$PACKAGE_JSON" | head -1 | cut -d'"' -f4)"
echo "  Chart.yaml:   $(grep '^version:' "$CHART_YAML" | head -1 | awk '{print $2}')"
echo ""

# Update Cargo.toml
echo -e "${YELLOW}[1/3]${NC} Updating backend/Cargo.toml..."
if [[ "$OSTYPE" == "darwin"* ]]; then
  # macOS
  sed -i '' "0,/^version = \".*\"/s//version = \"$VERSION\"/" "$CARGO_TOML"
else
  # Linux
  sed -i "0,/^version = \".*\"/s//version = \"$VERSION\"/" "$CARGO_TOML"
fi
echo "  ✅ Updated to $VERSION"

# Update package.json
echo -e "${YELLOW}[2/3]${NC} Updating frontend/package.json..."
if [[ "$OSTYPE" == "darwin"* ]]; then
  # macOS
  sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$PACKAGE_JSON"
else
  # Linux
  sed -i "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$PACKAGE_JSON"
fi
echo "  ✅ Updated to $VERSION"

# Update Chart.yaml (both version and appVersion)
echo -e "${YELLOW}[3/3]${NC} Updating deploy/chart/Chart.yaml..."
if [[ "$OSTYPE" == "darwin"* ]]; then
  # macOS
  sed -i '' "s/^version: .*/version: $VERSION/" "$CHART_YAML"
  sed -i '' "s/^appVersion: .*/appVersion: \"$VERSION\"/" "$CHART_YAML"
else
  # Linux
  sed -i "s/^version: .*/version: $VERSION/" "$CHART_YAML"
  sed -i "s/^appVersion: .*/appVersion: \"$VERSION\"/" "$CHART_YAML"
fi
echo "  ✅ Updated to $VERSION"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}✅ Version bumped to $VERSION${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""

# Show updated versions
echo -e "${YELLOW}Updated versions:${NC}"
echo "  Cargo.toml:   $(grep '^version' "$CARGO_TOML" | head -1 | cut -d'"' -f2)"
echo "  package.json: $(grep '"version"' "$PACKAGE_JSON" | head -1 | cut -d'"' -f4)"
echo "  Chart.yaml:   $(grep '^version:' "$CHART_YAML" | head -1 | awk '{print $2}')"
echo "  appVersion:   $(grep '^appVersion:' "$CHART_YAML" | head -1 | awk '{print $2}' | tr -d '"')"
echo ""

# Show next steps
echo -e "${YELLOW}Next steps:${NC}"
echo "  1. Update CHANGELOG.md with release notes"
echo "  2. Review changes: git diff"
echo "  3. Commit: git add . && git commit -m 'chore(release): prepare for v$VERSION'"
echo "  4. Tag: git tag v$VERSION"
echo "  5. Push: git push origin main && git push origin v$VERSION"
echo ""

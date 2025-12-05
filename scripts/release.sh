#!/bin/bash

# Release script for StarRocks Admin
# Usage: ./scripts/release.sh <version>
# Example: ./scripts/release.sh 1.0.0

set -e

VERSION=$1

if [ -z "$VERSION" ]; then
    echo "Error: Version number is required"
    echo "Usage: ./scripts/release.sh <version>"
    echo "Example: ./scripts/release.sh 1.0.0"
    exit 1
fi

# Validate version format (semantic versioning)
if ! [[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: Invalid version format. Use semantic versioning (e.g., 1.0.0)"
    exit 1
fi

TAG="v$VERSION"

echo "🚀 Preparing release $TAG"
echo ""

# Check if we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo "⚠️  Warning: You are not on the main branch (current: $CURRENT_BRANCH)"
    read -p "Do you want to continue? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo "❌ Error: You have uncommitted changes. Please commit or stash them first."
    exit 1
fi

# Pull latest changes
echo "📥 Pulling latest changes..."
git pull origin main

# Update version in Cargo.toml
echo "📝 Updating version in Cargo.toml..."
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" backend/Cargo.toml
rm -f backend/Cargo.toml.bak

# Update version in package.json
echo "📝 Updating version in package.json..."
cd frontend
npm version $VERSION --no-git-tag-version
cd ..

# Update CHANGELOG.md
echo "📝 Updating CHANGELOG.md..."
TODAY=$(date +%Y-%m-%d)
sed -i.bak "s/## \[Unreleased\]/## [Unreleased]\n\n## [$VERSION] - $TODAY/" CHANGELOG.md
rm -f CHANGELOG.md.bak

# Commit version changes
echo "💾 Committing version changes..."
git add backend/Cargo.toml frontend/package.json frontend/package-lock.json CHANGELOG.md
git commit -m "chore: bump version to $VERSION"

# Create and push tag
echo "🏷️  Creating tag $TAG..."
git tag -a "$TAG" -m "Release $TAG"

echo ""
echo "✅ Release preparation complete!"
echo ""
echo "Next steps:"
echo "1. Review the changes: git show HEAD"
echo "2. Push the changes: git push origin main"
echo "3. Push the tag: git push origin $TAG"
echo ""
echo "The GitHub Actions workflow will automatically:"
echo "  - Build the application for multiple platforms"
echo "  - Create a GitHub release"
echo "  - Upload release artifacts"
echo "  - Build and push Docker images"
echo "  - Publish Helm chart"
echo ""
echo "Or run all at once:"
echo "  git push origin main && git push origin $TAG"

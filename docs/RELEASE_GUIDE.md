# Release Guide

This guide explains how to create a new release for StarRocks Admin.

## Overview

The project uses GitHub Actions for automated releases. When you push a version tag (e.g., `v1.0.0`), the workflow automatically:

1. ✅ Builds the application for multiple platforms (Linux, macOS)
2. 📦 Creates release packages (.tar.gz)
3. 🐳 Builds and pushes Docker images to GitHub Container Registry
4. ⎈ Publishes Helm chart
5. 📝 Creates a GitHub Release with all artifacts

## Prerequisites

- You must have write access to the repository
- You must be on the `main` branch
- All changes must be committed
- All tests must pass

## Release Process

### Option 1: Using the Release Script (Recommended)

We provide a convenient script that automates the version bumping process:

```bash
# Create a new release (e.g., version 1.0.0)
./scripts/release.sh 1.0.0
```

This script will:
1. Validate the version format
2. Check you're on the main branch
3. Update version in `backend/Cargo.toml`
4. Update version in `frontend/package.json`
5. Update `CHANGELOG.md` with the release date
6. Commit the changes
7. Create a git tag
8. Show you the next steps

After running the script, push the changes:

```bash
# Push the commit and tag
git push origin main && git push origin v1.0.0
```

### Option 2: Manual Release

If you prefer to do it manually:

1. **Update version numbers:**

   ```bash
   # Update backend/Cargo.toml
   version = "1.0.0"
   
   # Update frontend/package.json
   cd frontend
   npm version 1.0.0 --no-git-tag-version
   cd ..
   ```

2. **Update CHANGELOG.md:**

   Add a new section for the release with today's date:
   ```markdown
   ## [1.0.0] - 2024-12-05
   
   ### Added
   - Feature 1
   - Feature 2
   
   ### Changed
   - Change 1
   
   ### Fixed
   - Bug fix 1
   ```

3. **Commit and tag:**

   ```bash
   git add backend/Cargo.toml frontend/package.json frontend/package-lock.json CHANGELOG.md
   git commit -m "chore: bump version to 1.0.0"
   git tag -a v1.0.0 -m "Release v1.0.0"
   ```

4. **Push to GitHub:**

   ```bash
   git push origin main
   git push origin v1.0.0
   ```

## Version Numbering

We follow [Semantic Versioning](https://semver.org/):

- **MAJOR** version (1.0.0 → 2.0.0): Incompatible API changes
- **MINOR** version (1.0.0 → 1.1.0): New features, backwards compatible
- **PATCH** version (1.0.0 → 1.0.1): Bug fixes, backwards compatible

Examples:
- `1.0.0` - First stable release
- `1.1.0` - Added new features
- `1.1.1` - Bug fixes
- `2.0.0` - Breaking changes

## Monitoring the Release

After pushing the tag, you can monitor the release process:

1. Go to the **Actions** tab in GitHub
2. Find the "Release" workflow run
3. Monitor the build progress for each platform
4. Once complete, check the **Releases** page

The workflow typically takes 15-30 minutes to complete.

## Release Artifacts

Each release includes:

### Binary Packages
- `starrocks-admin-linux-amd64.tar.gz` - Linux x86_64
- `starrocks-admin-macos-amd64.tar.gz` - macOS Intel
- `starrocks-admin-macos-arm64.tar.gz` - macOS Apple Silicon

### Docker Images (Built by docker-publish workflow)
- `ghcr.io/jlon/starrocks-admin:latest` - Latest stable release
- `ghcr.io/jlon/starrocks-admin:1.0.0` - Specific version
- `ghcr.io/jlon/starrocks-admin:1.0` - Minor version
- `ghcr.io/jlon/starrocks-admin:1` - Major version
- `ghcr.io/jlon/starrocks-admin:main` - Latest main branch build
- Multi-platform support: `linux/amd64`, `linux/arm64`

### Helm Chart
- `starrocks-admin-helm-1.0.0.tgz`

## Post-Release Checklist

After the release is published:

- [ ] Verify all artifacts are uploaded correctly
- [ ] Test the Docker image: `docker pull ghcr.io/jlon/starrocks-admin:latest`
- [ ] Test the binary package on at least one platform
- [ ] Update documentation if needed
- [ ] Announce the release (if applicable)
- [ ] Close related issues and PRs

## Troubleshooting

### Build Fails

If the build fails:
1. Check the Actions logs for error messages
2. Fix the issue in a new commit
3. Delete the tag: `git tag -d v1.0.0 && git push origin :refs/tags/v1.0.0`
4. Create a new tag after fixing

### Docker Push Fails

Ensure the `GITHUB_TOKEN` has the correct permissions:
- Go to Settings → Actions → General
- Enable "Read and write permissions" for workflows

### Release Already Exists

If you need to recreate a release:
1. Delete the release from GitHub UI
2. Delete the tag: `git tag -d v1.0.0 && git push origin :refs/tags/v1.0.0`
3. Create a new tag

## CI/CD Workflows

### CI Workflow (`.github/workflows/ci.yml`)

Runs on every push and PR to `main` and `develop` branches:
- Lints frontend and backend code
- Runs tests
- Builds the application
- Uploads build artifacts

### Release Workflow (`.github/workflows/release.yml`)

Runs when a version tag is pushed:
- Builds for multiple platforms (Linux, macOS Intel/ARM)
- Creates GitHub Release with detailed notes
- Uploads binary release artifacts (.tar.gz)
- Publishes Helm chart

### Docker Publish Workflow (`.github/workflows/docker-publish.yml`)

Runs automatically on:
- Version tag push (e.g., `v1.0.0`)
- Push to `main` branch
- Pull requests to `main`
- Manual trigger

Features:
- Multi-platform builds (linux/amd64, linux/arm64)
- Semantic versioning tags (e.g., `1.0.0`, `1.0`, `1`, `latest`)
- GitHub Container Registry (GHCR) publishing
- Build caching for faster builds

## Examples

### Creating a Patch Release

```bash
# Fix a bug
git commit -m "fix: resolve login issue"
git push origin main

# Create patch release
./scripts/release.sh 1.0.1
git push origin main && git push origin v1.0.1
```

### Creating a Minor Release

```bash
# Add new feature
git commit -m "feat: add new dashboard widget"
git push origin main

# Create minor release
./scripts/release.sh 1.1.0
git push origin main && git push origin v1.1.0
```

### Creating a Major Release

```bash
# Breaking changes
git commit -m "feat!: redesign API endpoints"
git push origin main

# Create major release
./scripts/release.sh 2.0.0
git push origin main && git push origin v2.0.0
```

## Support

If you encounter any issues with the release process, please:
1. Check the [GitHub Actions documentation](https://docs.github.com/en/actions)
2. Review the workflow logs
3. Open an issue in the repository

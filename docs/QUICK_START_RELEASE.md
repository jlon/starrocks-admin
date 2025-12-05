# Quick Start: Creating Your First Release

This is a quick guide to create your first release of StarRocks Admin.

## TL;DR

```bash
# 1. Make sure you're on main branch with latest changes
git checkout main
git pull origin main

# 2. Run the release script
./scripts/release.sh 0.1.0

# 3. Push everything
git push origin main && git push origin v0.1.0

# 4. Wait 15-30 minutes and check GitHub Releases page
```

That's it! GitHub Actions will handle the rest.

## What Happens Next?

After you push the tag, **two GitHub Actions workflows** will run automatically:

### 1. Release Workflow (`.github/workflows/release.yml`)

1. **Build binaries** for:
   - Linux (x86_64)
   - macOS (Intel)
   - macOS (Apple Silicon)

2. **Package Helm chart**:
   - `starrocks-admin-helm-0.1.0.tgz`

3. **Create GitHub Release** with:
   - Release notes
   - All binary packages
   - Helm chart
   - Installation instructions

### 2. Docker Publish Workflow (`.github/workflows/docker-publish.yml`)

1. **Build Docker images** for multiple platforms:
   - `linux/amd64` (Intel/AMD processors)
   - `linux/arm64` (ARM processors, Apple Silicon, AWS Graviton)

2. **Push to GitHub Container Registry** with tags:
   - `ghcr.io/jlon/starrocks-admin:latest`
   - `ghcr.io/jlon/starrocks-admin:0.1.0`
   - `ghcr.io/jlon/starrocks-admin:0.1`
   - `ghcr.io/jlon/starrocks-admin:0`

## Monitoring Progress

1. Go to: https://github.com/jlon/starrocks-admin/actions
2. Click on the "Release" workflow
3. Watch the build progress

## After Release

Once the workflow completes, you can:

### Test the Docker image:
```bash
docker pull ghcr.io/jlon/starrocks-admin:0.1.0
docker run -d -p 8080:8080 ghcr.io/jlon/starrocks-admin:0.1.0
```

### Download and test the binary:
```bash
wget https://github.com/jlon/starrocks-admin/releases/download/v0.1.0/starrocks-admin-linux-amd64.tar.gz
tar -xzf starrocks-admin-linux-amd64.tar.gz
cd starrocks-admin
./bin/starrocks-admin.sh start
```

### Install via Helm:
```bash
helm install starrocks-admin \
  https://github.com/jlon/starrocks-admin/releases/download/v0.1.0/starrocks-admin-helm-0.1.0.tgz
```

## Common Issues

### "Permission denied" when running release script
```bash
chmod +x scripts/release.sh
```

### "You have uncommitted changes"
```bash
git status
git add .
git commit -m "your message"
```

### "Not on main branch"
```bash
git checkout main
git pull origin main
```

## Next Steps

- Read the full [Release Guide](./RELEASE_GUIDE.md) for detailed information
- Update [CHANGELOG.md](../CHANGELOG.md) before each release
- Check [CI/CD workflows](../.github/workflows/) for customization

## Need Help?

- Check workflow logs in GitHub Actions
- Review [RELEASE_GUIDE.md](./RELEASE_GUIDE.md)
- Open an issue if you encounter problems

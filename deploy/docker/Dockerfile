# Multi-stage build for StarRocks Admin
FROM rust:1.75 as builder

# Install Node.js for frontend build
RUN curl -fsSL https://deb.nodesource.com/setup_18.x | bash - \
    && apt-get install -y nodejs

WORKDIR /app

# Copy backend source
COPY backend/ ./backend/
WORKDIR /app/backend

# Build backend
RUN cargo build --release

# Build frontend
WORKDIR /app
COPY frontend/ ./frontend/
WORKDIR /app/frontend

# Install dependencies and build
RUN npm ci --silent
RUN npm run build -- --configuration production
# Clean up dev dependencies to reduce image size
RUN npm prune --production

# Final stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false starrocks

# Create app directory structure
WORKDIR /app
RUN mkdir -p bin conf web lib data logs

# Copy built artifacts
COPY --from=builder /app/backend/target/release/starrocks-admin ./bin/
COPY --from=builder /app/frontend/dist/ ./web/
# Copy database migrations required at runtime
COPY --from=builder /app/backend/migrations ./migrations
# Create default config
RUN echo '[server]\nhost = "0.0.0.0"\nport = 8080\n\n[database]\nurl = "sqlite://data/starrocks-admin.db"\n\n[auth]\njwt_secret = "dev-secret-key-change-in-production"\njwt_expires_in = "24h"\n\n[cors]\nallow_origin = "http://localhost:4200"\n\n[logging]\nlevel = "info,starrocks_admin_backend=debug"\nfile = "logs/starrocks-admin.log"\n\n[static_config]\nenabled = true\nweb_root = "web"' > ./conf/config.toml

# Set permissions
RUN chown -R starrocks:starrocks /app
RUN chmod +x ./bin/starrocks-admin

# Switch to non-root user
USER starrocks

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Start command
CMD ["./bin/starrocks-admin"]

# Multi-stage build for StarRocks Admin
FROM rust:1.85.0 as builder

# Install Node.js for frontend build
RUN curl -fsSL https://deb.nodesource.com/setup_18.x | bash - \
    && apt-get install -y nodejs

WORKDIR /app

# Step 1: Build frontend first (required for embedding)
COPY frontend/ ./frontend/
WORKDIR /app/frontend

# Install dependencies and build (outputs to frontend/dist)
# Use relative base href (./) to support sub-path deployments (e.g., /starrocks-admin/)
# This follows Flink's approach for sub-path deployment support
# Note: Removed --silent to see detailed error messages during build
RUN npm ci
RUN npm run build -- --configuration production --base-href ./

# Step 2: Build backend (rust-embed will directly embed from ../frontend/dist)
WORKDIR /app
COPY backend/ ./backend/
WORKDIR /app/backend

# Build backend (rust-embed reads directly from ../frontend/dist, no copy needed)
RUN cargo build --release

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
RUN mkdir -p bin conf lib data logs

# Copy built artifacts
# Frontend is embedded in the binary (from frontend/dist during build)
COPY --from=builder /app/backend/target/release/starrocks-admin ./bin/

# Copy database migrations (required at runtime)
COPY --from=builder /app/backend/migrations ./migrations

# Create default config
RUN echo '[server]\nhost = "0.0.0.0"\nport = 8080\n\n[database]\nurl = "sqlite://data/starrocks-admin.db"\n\n[auth]\njwt_secret = "dev-secret-key-change-in-production"\njwt_expires_in = "24h"\n\n[logging]\nlevel = "info,starrocks_admin_backend=debug"\nfile = "logs/starrocks-admin.log"\n\n[metrics]\ninterval_secs = "30s"\nretention_days = "7d"\nenabled = true\n\n[audit]\ndatabase = "starrocks_audit_db__"\ntable = "starrocks_audit_tbl__"' > ./conf/config.toml

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

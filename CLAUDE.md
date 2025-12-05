# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**StarRocks Admin** - An enterprise-grade cluster management platform for StarRocks (high-performance analytical database). Full-stack TypeScript/Rust project with a modern Angular + Nebular UI and Rust/Axum backend.

- **Backend**: Rust (Axum web framework), SQLx with SQLite + MySQL connections
- **Frontend**: Angular 15, TypeScript, Nebular UI components
- **Architecture**: Monorepo with `/backend` and `/frontend` directories
- **Build**: Embedded frontend assets directly in Rust binary
- **Deployment**: Docker, Kubernetes, traditional shell scripts
- **Internationalization**: Multi-language support via `rust-i18n` (backend) and `@ngx-translate` (frontend)

---

## Quick Start for New Development

### Prerequisites
```bash
# Ensure you have Rust and Node.js installed
rustc --version  # 1.75+
node --version   # 16+
```

### First-Time Setup
```bash
# 1. Install frontend dependencies
cd frontend && npm install

# 2. Start backend dev server (requires SQLite at conf/config.toml)
cd backend && cargo run

# 3. In another terminal, start frontend dev server
cd frontend && npm run start

# 4. Access at http://localhost:4200
# Default backend API: http://localhost:8080
```

---

## Key Architecture Patterns

### Backend Design (Rust/Axum)

**Application State (`AppState`)**:
- Single shared state object containing all dependencies
- All services wrapped in `Arc` for cheap cloning across async tasks
- Design philosophy: Rust's type system IS the DI container—no service container anti-pattern
- Location: `backend/src/lib.rs:35-63`

**Service Layer Organization** (Domain-driven):
- **Database Services**: `MySQLPoolManager`, `CasbinService` (access control)
- **Domain Services**: `ClusterService`, `OrganizationService`, `AuthService`, `OverviewService`
- **System Services**: `MetricsCollectorService`, `DataStatisticsService`, `SystemFunctionService`, `LLMServiceImpl`
- **RBAC Services**: `PermissionService`, `RoleService`, `UserRoleService`, `UserService`
- Each service in `backend/src/services/` with corresponding models in `backend/src/models/`

**Request Handlers** (`backend/src/handlers/`):
- Organized by domain (e.g., `cluster.rs`, `query.rs`, `profile.rs`, `sql_diag.rs`)
- Handlers receive `AppState` extractor, validate input, call services
- Response types use Serde for JSON serialization

**Multi-Tenancy & Organization Filtering**:
- Middleware extracts `organization_id` from JWT claims
- Utility: `backend/src/utils/organization_filter.rs` provides SQL WHERE filters
- Services apply `org_filter` automatically to queries
- RBAC uses Casbin for fine-grained permission checks

**Profiles & Performance Analysis**:
- Profile parser: `backend/src/services/profile_analyzer/` handles query profile parsing
- Specialized parsers: scan, join, aggregate, exchange, sink operators
- Metrics extraction, topology parsing, tree building
- LLM integration: `backend/src/services/llm/` for AI-powered diagnostics

**Middleware Stack**:
- `backend/src/middleware/auth.rs`: JWT token validation
- `backend/src/middleware/permission_extractor.rs`: Extract org context from JWT
- `backend/src/middleware/locale.rs`: Language/i18n support (reads `Accept-Language` header)
- Tower layers for CORS, tracing, static file serving

**i18n Implementation** (Backend):
- Uses `rust-i18n` crate for compile-time i18n
- Translation files in `backend/locales/{lang}/` (e.g., `en`, `zh-CN`)
- All user-facing messages must use `t!` macro: `t!("error.cluster_not_found")`
- When adding new strings, add keys to appropriate locale files and ensure all locales are updated

### Frontend Design (Angular 15)

**Module Organization**:
- `@core/`: Core services, guards, data models
  - `services/`: API client, auth, permission, cluster context
  - `guards/`: Auth guard, permission guard
  - `data/`: Service classes for data access (cluster, user, role, etc.)
  - `interceptors/`: HTTP interceptors including language selection
- `@theme/`: UI components, layouts, pipes, styles
  - `components/`: Search input, footer, TinyMCE editor
  - `pipes/`: Custom pipes (capitalize, number-with-commas, timing, etc.)
  - `layouts/`: One/two/three-column layouts
- `pages/`: Feature modules organized by domain
  - `starrocks/`: Cluster management, queries, profiles, materialized views
  - `system/`: User management, roles, organizations
- `auth/`: Authentication pages

**Key Services**:
- `@core/data/api.service.ts`: Base HTTP client with interceptors (handles `Accept-Language` header)
- `@core/data/cluster-context.service.ts`: Selected cluster state (BehaviorSubject)
- `@core/interceptors/language.interceptor.ts`: Injects language header for i18n
- `@core/routing/tab-route-reuse.strategy.ts`: Smart tab/page reuse

**State Management Pattern**:
- RxJS services + BehaviorSubject for reactive state
- Services manage data loading, caching, and updates
- Components subscribe via async pipe (preferred) or manual subscription with unsubscribe in `ngOnDestroy`
- Avoid nested subscriptions—use RxJS operators like `switchMap`, `combineLatest`

**i18n Implementation** (Frontend):
- Uses `@ngx-translate` library
- Translation files in `frontend/src/assets/i18n/{lang}.json` (e.g., `en.json`, `zh-CN.json`)
- Use `{{ 'key.name' | translate }}` in templates
- Use `this.translateService.get('key.name')` in components when string is needed in logic
- Language selection is synced with backend via header injection

---

## Common Development Commands

### Build & Deploy

```bash
# Full build (frontend + backend)
make build

# Build only frontend
bash build/build-frontend.sh

# Build only backend
bash build/build-backend.sh

# Build and run Docker
make docker-build
make docker-up
make docker-down

# Clean all artifacts
make clean
```

### Backend Development

```bash
# Check compilation (without full build)
cd backend && cargo check

# Lint with clippy (strict mode - used in build)
cd backend && cargo clippy --release --all-targets -- --deny warnings --allow clippy::uninlined-format-args

# Run with debug logging
cd backend && RUST_LOG=debug cargo run

# Run tests
cd backend && cargo test

# Run specific test
cd backend && cargo test test_name -- --nocapture

# Format code
cd backend && cargo fmt

# Generate API docs
cd backend && cargo doc --no-deps --open
```

### Frontend Development

```bash
# Install dependencies
cd frontend && npm install

# Development server (with live reload on localhost:4200)
cd frontend && npm run start

# Build for production
cd frontend && npm run build:prod

# Run tests
cd frontend && npm run test

# Lint & fix code
cd frontend && npm run lint:fix

# Lint styles
cd frontend && npm run lint:styles

# Generate documentation
cd frontend && npm run docs
```

### Database & Configuration

- **SQLite**: `conf/config.toml` defines `database.url` for development (default: `sqlite://data/starrocks-admin.db`)
- **Migrations**: `backend/migrations/` contains SQL migration files (auto-run on startup via SQLx)
- **Configuration**: `conf/config.toml` has sections for `[server]`, `[database]`, `[auth]`, `[logging]`, `[metrics]`, `[audit]`, `[i18n]`
- **Environment Overrides**: Config values can be overridden via `APP_*` env vars (e.g., `APP_SERVER_PORT=9000`)

---

## Development Workflow

### Key File Locations for Common Tasks

**Adding a New API Endpoint**:
1. Define handler in `backend/src/handlers/{domain}.rs`
2. Add Axum route in `backend/src/main.rs`
3. Create service method in `backend/src/services/{domain}_service.rs`
4. Define models in `backend/src/models/{domain}.rs`
5. Frontend: Create service in `frontend/src/app/@core/data/` and component in `frontend/src/app/pages/`
6. Add i18n strings: Rust error messages use `t!()` macro, Angular templates use `| translate` pipe

**Adding i18n Support to New Features**:
1. Backend: Add string keys to `backend/locales/en/main.rs` and corresponding locale files
2. Frontend: Add keys to `frontend/src/assets/i18n/en.json` and other locale files
3. Backend: Use `t!("key.name")` in error messages
4. Frontend: Use `{{ 'key.name' | translate }}` in templates or `this.translateService.get('key.name')` in components

**Modifying Database Schema**:
1. Create migration in `backend/migrations/YYYYMMDD_*.sql`
2. Models updated in `backend/src/models/`
3. Service layer queries updated accordingly

**Adding New UI Component**:
1. Create component in appropriate `pages/` folder with `.component.ts`, `.component.html`, `.component.scss`
2. Declare in module's `declarations` and `imports`
3. Add routing if needed in feature routing module
4. Add i18n keys for any user-facing text

**Multi-Tenant Considerations**:
- Always apply `org_filter()` in service queries to filter by organization
- Check middleware applies `organization_id` extraction
- RBAC checks via Casbin service for fine-grained permissions
- Frontend components should use `ClusterContextService` to get selected cluster context

---

## Code Quality Standards

### Rust Backend

- **Linting**: `cargo clippy --release --all-targets -- --deny warnings` (enforced in build)
- **Formatting**: `cargo fmt` (automatic)
- **Testing**: Unit tests in modules (suffix `#[cfg(test)]`), integration tests in `backend/src/tests/`
- **Error Handling**: Use `anyhow::Result<T>` for app errors, custom error types with `thiserror`
- **Async**: All I/O is async via Tokio runtime—never block
- **Safety**: Prefer `Arc` over raw pointers; use `String` over `&'static str` for owned data
- **Principles**: Follow KISS (simple), YAGNI (no over-engineering), DRY (no duplication), SOLID (single responsibility)
- **Code Size**: Each method max ~100 lines; split large functions into smaller ones

### TypeScript Frontend

- **Linting**: `npm run lint` (ESLint + template linter)
- **Style Linting**: `npm run lint:styles` (Stylelint)
- **Formatting**: Should follow ESLint rules
- **Testing**: `npm run test` (Jasmine + Karma)
- **Type Safety**: Strict TypeScript mode enabled; avoid `any` type
- **RxJS**: Use reactive patterns with Observable/Subject; unsubscribe in `ngOnDestroy`
- **Change Detection**: Use `OnPush` strategy for tables/lists with large data
- **Principles**: Apply KISS, YAGNI, DRY, SOLID (SRP, OCP, ISP)—favor composition over inheritance
- **Code Size**: Each method max ~100 lines; extract helper methods liberally

---

## Recent Development (Last Commits)

The project has recently completed **internationalization (i18n)**:
- i18n library: Rust (`rust-i18n` crate) and Angular (`@ngx-translate`)
- Added language interceptor for frontend
- Added locale middleware for backend
- Locales in `backend/locales/` directory
- All user-facing messages and toastr prompts translated
- Frontend locale files in `frontend/src/assets/i18n/`

This indicates active development on UX improvements and multi-language support.

---

## Performance Considerations

- **Metrics Collection**: `MetricsCollectorService` runs on configurable intervals (default 30s), stores for retention period (default 7d)
- **Profile Analysis**: Heavy parsing via Profile Analyzer—consider memoization for repeated profiles
- **Database Queries**: Use indexes on frequently filtered columns (organization_id, cluster_id)
- **Frontend**: Lazy-load feature modules, use change detection strategy OnPush for tables, unsubscribe from observables
- **Async I/O**: All database/HTTP calls are async—never block Tokio runtime
- **Bundle Size**: Tree-shake unused code; Angular lazy loading for feature modules

---

## Security Notes

- **JWT Authentication**: Configured in `backend/src/middleware/auth.rs`; tokens contain `user_id` and `organization_id`
- **Database**: SQLite for local development; MySQL for production (configurable)
- **Access Control**: Casbin RBAC for role-based permissions
- **SQL Injection**: Use SQLx query macros (compile-time checked) for all database access—never string interpolation
- **CORS**: Configured via Tower middleware in `backend/src/main.rs`
- **StarRocks Credentials**: Stored per cluster in database; connections pooled per cluster
- **Configuration Secrets**: Never commit `.env` or `config.toml` with real secrets; use environment variables in production

---

## Useful Patterns

### Service Dependency Injection Pattern (Rust)
```rust
// In main.rs or lib.rs
let app_state = AppState {
    db: sqlite_pool,
    auth_service: Arc::new(AuthService::new(db_pool, jwt_util.clone())),
    cluster_service: Arc::new(ClusterService::new(db_pool, mysql_pool_manager.clone())),
    // ... more services
};
```

### Handler Pattern (Rust)
```rust
pub async fn get_clusters(
    State(state): State<AppState>,
    auth: AuthExtractor,  // JWT claims
) -> JsonResponse<Vec<Cluster>> {
    // Validate, call service, return response
    let org_filter = org_filter(&auth.org_id);
    let clusters = state.cluster_service.list_by_org(&org_filter).await?;
    ok(clusters)
}
```

### API Service Pattern (Angular)
```typescript
// Service calls backend with language header
export class ClusterService {
  constructor(private api: ApiService) {} // ApiService injects language header

  listClusters(): Observable<Cluster[]> {
    return this.api.get<Cluster[]>('/api/clusters');
  }
}

// Component with proper subscription management
export class ClusterListComponent implements OnInit, OnDestroy {
  private destroy$ = new Subject<void>();
  clusters$ = this.clusterService.listClusters();

  constructor(private clusterService: ClusterService) {}

  ngOnDestroy() {
    this.destroy$.next();
    this.destroy$.complete();
  }
}
```

---

## Debugging Tips

- **Backend Logs**: `tail -f logs/starrocks-admin.log` (set `RUST_LOG=debug` for verbose output)
- **Frontend Console**: Browser DevTools > Console for runtime errors
- **Database**: SQLite CLI `sqlite3 data/starrocks-admin.db`; MySQL CLI for production
- **Network**: Browser DevTools > Network for API request/response inspection
- **Rust Backtraces**: `RUST_BACKTRACE=full cargo run`
- **Angular Change Detection**: Enable debug mode in `main.ts`: `enableDebugTools(componentRef);`
- **i18n Debug**: Check `Accept-Language` header in Network tab; check locale JSON files are valid

---

## Known Limitations & TODOs

- Profile analysis may time out on very large profiles (10K+ operators)
- Metrics retention is in-memory; restart clears historical data (should migrate to DB)
- Frontend table virtualization for large datasets not yet implemented
- Real-time WebSocket support not yet added (polling for now)
- Performance optimization opportunity: Cache frequently accessed cluster metadata

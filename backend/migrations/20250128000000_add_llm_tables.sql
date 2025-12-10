-- ============================================================================
-- LLM Service Tables and Permissions
-- Provides LLM-enhanced analysis capabilities for StarRocks Admin
-- Created: 2025-01-28
-- Updated: 2025-01-29 (merged with permissions)
-- ============================================================================

-- ============================================================================
-- SECTION 1: LLM TABLES
-- ============================================================================

-- ============================================================================
-- 1.1 LLM Provider Configuration
-- Stores API configuration for different LLM providers (OpenAI, Azure, DeepSeek, etc.)
-- ============================================================================
CREATE TABLE IF NOT EXISTS llm_providers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,                    -- Provider name, e.g., "openai", "azure", "deepseek"
    display_name TEXT NOT NULL,                   -- Display name, e.g., "OpenAI GPT-4"
    api_base TEXT NOT NULL,                       -- API base URL
    model_name TEXT NOT NULL,                     -- Model name, e.g., "gpt-4o", "deepseek-chat"
    api_key_encrypted TEXT,                       -- Encrypted API key (AES-256 in production)
    is_active BOOLEAN DEFAULT FALSE,              -- Whether this provider is ACTIVE for use (only ONE can be active)
    max_tokens INTEGER DEFAULT 4096,              -- Maximum tokens for response
    temperature REAL DEFAULT 0.3,                 -- Temperature for generation
    timeout_seconds INTEGER DEFAULT 60,           -- Request timeout
    enabled BOOLEAN DEFAULT TRUE,                 -- Whether this provider is enabled (can be activated)
    priority INTEGER DEFAULT 100,                 -- Priority for fallback (lower = higher priority)
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    CHECK (is_active IN (0, 1))
);

-- Active provider index (for quick lookup)
CREATE INDEX IF NOT EXISTS idx_llm_providers_active ON llm_providers(is_active) WHERE is_active = 1;
CREATE INDEX IF NOT EXISTS idx_llm_providers_enabled ON llm_providers(enabled, priority);

-- ============================================================================
-- 1.2 LLM Analysis Sessions
-- Tracks each LLM analysis request for monitoring and debugging
-- ============================================================================
CREATE TABLE IF NOT EXISTS llm_analysis_sessions (
    id TEXT PRIMARY KEY,                          -- UUID
    provider_id INTEGER REFERENCES llm_providers(id),
    scenario TEXT NOT NULL DEFAULT 'root_cause_analysis',  -- Analysis scenario type
    query_id TEXT NOT NULL,                       -- StarRocks query ID
    cluster_id INTEGER,                           -- Cluster ID if applicable
    status TEXT NOT NULL DEFAULT 'pending',       -- pending/processing/completed/failed
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP,
    input_tokens INTEGER,                         -- Token count for input
    output_tokens INTEGER,                        -- Token count for output
    latency_ms INTEGER,                           -- Total latency in milliseconds
    error_message TEXT,                           -- Error message if failed
    retry_count INTEGER DEFAULT 0                 -- Number of retries
);

-- Index for query lookup
CREATE INDEX IF NOT EXISTS idx_llm_sessions_query ON llm_analysis_sessions(query_id);
CREATE INDEX IF NOT EXISTS idx_llm_sessions_status ON llm_analysis_sessions(status, created_at);
CREATE INDEX IF NOT EXISTS idx_llm_sessions_cluster ON llm_analysis_sessions(cluster_id, created_at);
CREATE INDEX IF NOT EXISTS idx_llm_sessions_scenario ON llm_analysis_sessions(scenario);

-- ============================================================================
-- 1.3 LLM Analysis Requests
-- Stores the input data sent to LLM (for debugging and replay)
-- ============================================================================
CREATE TABLE IF NOT EXISTS llm_analysis_requests (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES llm_analysis_sessions(id) ON DELETE CASCADE,
    request_json TEXT NOT NULL,                   -- Full request JSON
    sql_hash TEXT NOT NULL,                       -- Hash of SQL statement (for deduplication)
    profile_hash TEXT NOT NULL,                   -- Hash of profile key metrics
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for cache lookup
CREATE INDEX IF NOT EXISTS idx_llm_requests_session ON llm_analysis_requests(session_id);
CREATE INDEX IF NOT EXISTS idx_llm_requests_hash ON llm_analysis_requests(sql_hash, profile_hash);

-- ============================================================================
-- 1.4 LLM Analysis Results
-- Stores the parsed LLM response
-- ============================================================================
CREATE TABLE IF NOT EXISTS llm_analysis_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL UNIQUE REFERENCES llm_analysis_sessions(id) ON DELETE CASCADE,
    root_causes_json TEXT NOT NULL DEFAULT '[]',  -- JSON array of root causes
    causal_chains_json TEXT NOT NULL DEFAULT '[]', -- JSON array of causal chains
    recommendations_json TEXT NOT NULL DEFAULT '[]', -- JSON array of recommendations
    summary TEXT NOT NULL DEFAULT '',             -- Natural language summary
    hidden_issues_json TEXT DEFAULT '[]',         -- JSON array of hidden issues
    confidence_avg REAL,                          -- Average confidence score
    root_cause_count INTEGER,                     -- Number of root causes identified
    recommendation_count INTEGER,                 -- Number of recommendations
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for session lookup
CREATE INDEX IF NOT EXISTS idx_llm_results_session ON llm_analysis_results(session_id);
CREATE INDEX IF NOT EXISTS idx_llm_results_confidence ON llm_analysis_results(confidence_avg);

-- ============================================================================
-- 1.5 LLM Response Cache
-- Caches LLM responses to avoid redundant API calls for similar queries
-- ============================================================================
CREATE TABLE IF NOT EXISTS llm_cache (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cache_key TEXT NOT NULL UNIQUE,               -- Cache key (hash of normalized request)
    scenario TEXT NOT NULL DEFAULT 'root_cause_analysis',  -- Analysis scenario
    request_hash TEXT NOT NULL,                   -- Hash of the request
    response_json TEXT NOT NULL,                  -- Cached response JSON
    hit_count INTEGER DEFAULT 0,                  -- Number of cache hits
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP NOT NULL,                -- Cache expiration time
    last_accessed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for cache lookup and cleanup
CREATE INDEX IF NOT EXISTS idx_llm_cache_key ON llm_cache(cache_key);
CREATE INDEX IF NOT EXISTS idx_llm_cache_expires ON llm_cache(expires_at);
CREATE INDEX IF NOT EXISTS idx_llm_cache_scenario ON llm_cache(scenario);

-- ============================================================================
-- 1.6 LLM Usage Statistics (Aggregated)
-- Daily aggregated statistics for monitoring and cost tracking
-- ============================================================================
CREATE TABLE IF NOT EXISTS llm_usage_stats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,                           -- Statistics date (YYYY-MM-DD)
    provider_id INTEGER REFERENCES llm_providers(id),
    total_requests INTEGER DEFAULT 0,             -- Total API requests
    successful_requests INTEGER DEFAULT 0,        -- Successful requests
    failed_requests INTEGER DEFAULT 0,            -- Failed requests
    total_input_tokens INTEGER DEFAULT 0,         -- Total input tokens
    total_output_tokens INTEGER DEFAULT 0,        -- Total output tokens
    avg_latency_ms REAL,                          -- Average latency
    cache_hits INTEGER DEFAULT 0,                 -- Cache hit count
    estimated_cost_usd REAL,                      -- Estimated cost in USD
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(date, provider_id)
);

-- Index for date range queries
CREATE INDEX IF NOT EXISTS idx_llm_usage_date ON llm_usage_stats(date, provider_id);

-- ============================================================================
-- 1.7 Triggers for automatic timestamp updates
-- ============================================================================
CREATE TRIGGER IF NOT EXISTS update_llm_providers_timestamp
AFTER UPDATE ON llm_providers
BEGIN
    UPDATE llm_providers SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- ============================================================================
-- SECTION 2: DEFAULT DATA
-- ============================================================================

-- ============================================================================
-- 2.1 Insert default DeepSeek provider (inactive by default)
-- ============================================================================
INSERT OR IGNORE INTO llm_providers (name, display_name, api_base, model_name, is_active, enabled, priority)
VALUES ('deepseek', 'DeepSeek Chat', 'https://api.deepseek.com/v1', 'deepseek-chat', FALSE, TRUE, 1);

-- ============================================================================
-- SECTION 3: LLM PERMISSIONS
-- ============================================================================

-- ============================================================================
-- 3.1 Add LLM Menu Permission
-- ============================================================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
('menu:system:llm', 'LLM管理', 'menu', 'system:llm', 'view', '查看LLM管理');

-- ============================================================================
-- 3.2 Add LLM API Permissions
-- ============================================================================
INSERT OR IGNORE INTO permissions (code, name, type, resource, action, description) VALUES
-- LLM Status
('api:llm:status', 'LLM服务状态', 'api', 'llm', 'status', 'GET /api/llm/status'),
-- LLM Providers CRUD
('api:llm:providers:list', 'LLM提供商列表', 'api', 'llm', 'providers:list', 'GET /api/llm/providers'),
('api:llm:providers:get', '查看LLM提供商', 'api', 'llm', 'providers:get', 'GET /api/llm/providers/:id'),
('api:llm:providers:active', '获取活跃LLM提供商', 'api', 'llm', 'providers:active', 'GET /api/llm/providers/active'),
('api:llm:providers:create', '创建LLM提供商', 'api', 'llm', 'providers:create', 'POST /api/llm/providers'),
('api:llm:providers:update', '更新LLM提供商', 'api', 'llm', 'providers:update', 'PUT /api/llm/providers/:id'),
('api:llm:providers:delete', '删除LLM提供商', 'api', 'llm', 'providers:delete', 'DELETE /api/llm/providers/:id'),
('api:llm:providers:activate', '激活LLM提供商', 'api', 'llm', 'providers:activate', 'POST /api/llm/providers/:id/activate'),
('api:llm:providers:deactivate', '停用LLM提供商', 'api', 'llm', 'providers:deactivate', 'POST /api/llm/providers/:id/deactivate'),
('api:llm:providers:test', '测试LLM连接', 'api', 'llm', 'providers:test', 'POST /api/llm/providers/:id/test'),
-- LLM Analysis
('api:llm:analyze:root-cause', 'LLM根因分析', 'api', 'llm', 'analyze:root-cause', 'POST /api/llm/analyze/root-cause');

-- ============================================================================
-- 3.3 Set Parent ID for LLM API Permissions
-- ============================================================================
-- Associate LLM API permissions with menu:system:llm
UPDATE permissions 
SET parent_id = (SELECT id FROM permissions WHERE code = 'menu:system:llm')
WHERE code LIKE 'api:llm:%';

-- ============================================================================
-- 3.4 Grant LLM Permissions to Admin Role
-- ============================================================================
-- Ensure admin role has all LLM permissions
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
SELECT (SELECT id FROM roles WHERE code='admin'), id 
FROM permissions 
WHERE code LIKE 'menu:system:llm' OR code LIKE 'api:llm:%';

-- ============================================================================
-- MIGRATION COMPLETE
-- ============================================================================
-- Tables Created (6):
--   1. llm_providers          - LLM provider configuration
--   2. llm_analysis_sessions  - Analysis session tracking
--   3. llm_analysis_requests  - Request data storage
--   4. llm_analysis_results   - Analysis results
--   5. llm_cache              - Response cache
--   6. llm_usage_stats        - Usage statistics
--
-- Default Data:
--   - DeepSeek provider (inactive by default)
--
-- Permissions Added:
--   - 1 menu permission: menu:system:llm
--   - 11 API permissions for LLM management
--   - All permissions assigned to admin role

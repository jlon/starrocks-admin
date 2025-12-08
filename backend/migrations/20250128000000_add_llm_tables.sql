-- ============================================================================
-- LLM Service Tables
-- Provides LLM-enhanced analysis capabilities for StarRocks Admin
-- Created: 2025-01-28
-- ============================================================================

-- ============================================================================
-- LLM Provider Configuration
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
-- LLM Analysis Sessions
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
-- LLM Analysis Requests
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
-- LLM Analysis Results
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
-- LLM Response Cache
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
-- LLM Usage Statistics (Aggregated)
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
-- Triggers for automatic timestamp updates
-- ============================================================================
CREATE TRIGGER IF NOT EXISTS update_llm_providers_timestamp
AFTER UPDATE ON llm_providers
BEGIN
    UPDATE llm_providers SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;

-- ============================================================================
-- Insert default DeepSeek provider (inactive by default)
-- ============================================================================
INSERT OR IGNORE INTO llm_providers (name, display_name, api_base, model_name, is_active, enabled, priority)
VALUES ('deepseek', 'DeepSeek Chat', 'https://api.deepseek.com/v1', 'deepseek-chat', FALSE, TRUE, 1);

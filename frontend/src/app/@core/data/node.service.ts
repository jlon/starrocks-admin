import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiService } from './api.service';

export interface Backend {
  // Common fields for both BE and CN
  BackendId: string;           // Node ID (BackendId for BE, ComputeNodeId for CN)
  IP: string;                  // IP address
  HeartbeatPort: string;       // Heartbeat port
  BePort: string;              // BE service port
  HttpPort: string;            // HTTP port
  BrpcPort: string;            // BRPC port
  LastStartTime: string;       // Last start time
  LastHeartbeat: string;       // Last heartbeat time
  Alive: string;               // Alive status
  SystemDecommissioned: string; // System decommissioned status
  ClusterDecommissioned: string; // Cluster decommissioned status
  
  // Storage fields (BE only, CN may have empty values)
  TabletNum: string;           // Tablet count
  DataUsedCapacity: string;    // Data used capacity
  AvailCapacity: string;       // Available capacity
  TotalCapacity: string;       // Total capacity
  UsedPct: string;             // Used percentage
  MaxDiskUsedPct: string;      // Max disk used percentage
  DataTotalCapacity: string;   // Data total capacity
  DataUsedPct: string;         // Data used percentage
  
  // Resource fields
  CpuCores: string;            // CPU cores
  MemLimit: string;            // Memory limit
  NumRunningQueries: string;   // Number of running queries
  MemUsedPct: string;          // Memory used percentage
  CpuUsedPct: string;          // CPU used percentage
  
  // Status fields
  ErrMsg: string;              // Error message
  Version: string;             // Version
  Status: string;              // Status (JSON format)
  DataCacheMetrics: string;    // Data cache metrics
  Location: string;            // Location (BE only)
  StatusCode: string;          // Status code
  HasStoragePath: string;      // Has storage path (CN only)
  
  // Shared-Data mode fields
  StarletPort: string;         // Starlet port
  WorkerId: string;            // Worker ID
  WarehouseName: string;       // Warehouse name
}

export interface Frontend {
  Id?: string;  // Optional field added in StarRocks 3.5.2
  Name: string;
  IP: string;  // Changed from Host to IP to match StarRocks API
  EditLogPort: string;
  HttpPort: string;
  QueryPort: string;
  RpcPort: string;
  Role: string;
  IsMaster?: string;  // Made optional as it might not always be present
  ClusterId: string;
  Join: string;
  Alive: string;
  ReplayedJournalId: string;
  LastHeartbeat: string;
  IsHelper?: string;  // Optional field added in StarRocks 3.5.2
  ErrMsg: string;
  StartTime?: string;  // Optional field added in StarRocks 3.5.2
  Version: string;
}

export interface Query {
  QueryId: string;
  ConnectionId: string;
  Database: string;
  User: string;
  ScanBytes: string;
  ProcessRows: string;
  CPUTime: string;
  ExecTime: string;
  Sql: string;
  StartTime?: string;
  feIp?: string;
  MemoryUsage?: string;
  DiskSpillSize?: string;
  ExecProgress?: string;
  Warehouse?: string;
  CustomQueryId?: string;
  ResourceGroup?: string;
}

export interface SystemFunction {
  name: string;
  description: string;
  category: string;
  status: string;
  last_updated: string;
}

export interface SystemFunctionDetail {
  function_name: string;
  description: string;
  data: any[];
  total_count: number;
  last_updated: string;
}

export interface Session {
  id: string;
  user: string;
  host: string;
  db: string | null;
  command: string;
  time: string;
  state: string;
  info: string | null;
}

export interface Variable {
  name: string;
  value: string;
}

export interface VariableUpdateRequest {
  value: string;
  scope: string; // 'GLOBAL' or 'SESSION'
}

export interface SqlBlacklistItem {
  Id: string;
  Pattern: string;
}

export interface QueryHistoryItem {
  query_id: string;
  user: string;
  default_db: string;
  sql_statement: string;
  query_type: string;
  start_time: string;
  end_time: string;
  total_ms: number;
  query_state: string;
  warehouse: string;
}

export interface QueryHistoryResponse {
  data: QueryHistoryItem[];
  total: number;
  page: number;
  page_size: number;
}

export interface QueryProfile {
  query_id: string;
  sql: string;
  profile_content: string;
  execution_time_ms: number;
  status: string;
  fragments: any[];
}

export interface QueryExecuteRequest {
  sql: string;
  limit?: number;
  catalog?: string;
  database?: string;
}

export interface SingleQueryResult {
  sql: string;
  columns: string[];
  rows: string[][];
  row_count: number;
  execution_time_ms: number;
  success: boolean;
  error?: string;
}

export interface QueryExecuteResult {
  results: SingleQueryResult[];
  total_execution_time_ms: number;
}

export type TableObjectType = 'TABLE' | 'VIEW' | 'MATERIALIZED_VIEW';

export interface TableInfo {
  name: string;
  object_type: TableObjectType;
}

export interface ProfileListItem {
  QueryId: string;
  StartTime: string;
  Time: string;
  State: string;
  Statement: string;
}

export interface ProfileDetail {
  query_id: string;
  profile_content: string;
}

export interface ProfileAnalysisNode {
  id: string;
  operator_name: string;
  node_type: string;
  plan_node_id: number;
  parent_plan_node_id: number | null;
  metrics: any;
  children: string[];
  depth: number;
  is_hotspot: boolean;
  hotspot_severity: string;
  fragment_id: string;
  pipeline_id: string;
  time_percentage: number;
  is_most_consuming: boolean;
  is_second_most_consuming: boolean;
  unique_metrics: any;
  // Whether this node has diagnostic issues (for UI warning indicator)
  has_diagnostic?: boolean;
  // List of diagnostic rule IDs associated with this node
  diagnostic_ids?: string[];
}

export interface ProfileExecutionTree {
  root: ProfileAnalysisNode;
  nodes: ProfileAnalysisNode[];
}

// Diagnostic result from rule engine
export interface DiagnosticResult {
  rule_id: string;
  rule_name: string;
  severity: string;
  node_path: string;
  // Plan node ID for associating with execution tree node
  plan_node_id?: number;
  // Summary of the diagnostic issue (诊断结果概要)
  message: string;
  // Detailed explanation of why this issue occurs (详细诊断原因)
  reason: string;
  // Recommended actions to fix the issue (建议措施)
  suggestions: string[];
  parameter_suggestions?: ParameterSuggestion[];
}

// Parameter suggestion with description and impact
export interface ParameterSuggestion {
  name: string;
  param_type: string;
  current?: string;
  recommended: string;
  command: string;
  description: string;  // Human-readable description of what this parameter does
  impact: string;       // Expected impact of changing this parameter
}

// Aggregated diagnostic for overview display
export interface AggregatedDiagnostic {
  rule_id: string;
  rule_name: string;
  severity: string;
  // Aggregated summary message
  message: string;
  // Detailed explanation
  reason: string;
  // List of affected node paths
  affected_nodes: string[];
  // Number of affected nodes
  node_count: number;
  // Merged suggestions (deduplicated)
  suggestions: string[];
  parameter_suggestions?: ParameterSuggestion[];
}

// LLM enhanced analysis result
export interface LLMEnhancedAnalysis {
  available: boolean;
  status: string;  // 'pending' | 'completed' | 'failed'
  root_causes?: any[];
  causal_chains?: any[];
  recommendations?: any[];
  hidden_issues?: any[];
  summary?: string;
  from_cache?: boolean;  // Whether this result was from cache
  elapsed_time_ms?: number;  // LLM analysis elapsed time in milliseconds
}

export interface ProfileAnalysisResult {
  hotspots: any[];
  conclusion: string;
  suggestions: string[];
  performance_score: number;
  execution_tree: ProfileExecutionTree;
  summary: {
    query_id: string;
    top_time_consuming_nodes: Array<{
      operator_name: string;
      time_percentage: number;
    }>;
    [key: string]: any;
  };
  // Rule-based diagnostics with detailed reasons (all diagnostics)
  diagnostics?: DiagnosticResult[];
  // Aggregated diagnostics by rule_id for overview display
  aggregated_diagnostics?: AggregatedDiagnostic[];
  // Node-level diagnostics mapping (plan_node_id -> diagnostics)
  node_diagnostics?: { [planNodeId: number]: DiagnosticResult[] };
  // Raw profile content for PROFILE tab display
  profile_content?: string;
  // LLM enhanced analysis (loaded async after DAG)
  llm_analysis?: LLMEnhancedAnalysis;
  // Rule-based root cause analysis
  root_cause_analysis?: any;
}

@Injectable({
  providedIn: 'root',
})
export class NodeService {
  constructor(private api: ApiService) {}

  // All API methods now use backend routes without cluster ID
  // The active cluster is determined by the backend
  
  listBackends(): Observable<Backend[]> {
    return this.api.get<Backend[]>(`/clusters/backends`);
  }

  deleteBackend(host: string, port: string): Observable<any> {
    return this.api.delete<any>(`/clusters/backends/${host}/${port}`);
  }

  listFrontends(): Observable<Frontend[]> {
    return this.api.get<Frontend[]>(`/clusters/frontends`);
  }

  listQueries(): Observable<Query[]> {
    return this.api.get<Query[]>(`/clusters/queries`);
  }

  killQuery(queryId: string): Observable<any> {
    return this.api.delete(`/clusters/queries/${queryId}`);
  }

  // SQL Blacklist API
  listSqlBlacklist(): Observable<SqlBlacklistItem[]> {
    return this.api.get<SqlBlacklistItem[]>(`/clusters/sql-blacklist`);
  }

  addSqlBlacklist(pattern: string): Observable<any> {
    return this.api.post(`/clusters/sql-blacklist`, { pattern });
  }

  deleteSqlBlacklist(id: string): Observable<any> {
    return this.api.delete(`/clusters/sql-blacklist/${id}`);
  }

  getSystemFunctions(): Observable<SystemFunction[]> {
    return this.api.get<SystemFunction[]>(`/clusters/system`);
  }

  getSystemFunctionDetail(functionName: string, nestedPath?: string): Observable<SystemFunctionDetail> {
    const url = nestedPath 
      ? `/clusters/system/${functionName}?path=${encodeURIComponent(nestedPath)}`
      : `/clusters/system/${functionName}`;
    return this.api.get<SystemFunctionDetail>(url);
  }

  // Sessions API
  getSessions(): Observable<Session[]> {
    return this.api.get<Session[]>(`/clusters/sessions`);
  }

  killSession(sessionId: string): Observable<any> {
    return this.api.delete(`/clusters/sessions/${sessionId}`);
  }

  // Variables API
  getVariables(type: string = 'global', filter?: string): Observable<Variable[]> {
    let params: any = { type };
    if (filter) {
      params.filter = filter;
    }
    return this.api.get<Variable[]>(`/clusters/variables`, params);
  }

  getConfigureInfo(): Observable<Variable[]> {
    return this.api.get<Variable[]>(`/clusters/configs`);
  }

  updateVariable(variableName: string, request: VariableUpdateRequest): Observable<any> {
    return this.api.put(`/clusters/variables/${variableName}`, request);
  }

  // Query History API with pagination and search
  listQueryHistory(limit: number = 10, offset: number = 0, filters?: {
    keyword?: string;
    startTime?: string;
    endTime?: string;
  }): Observable<QueryHistoryResponse> {
    const params: any = { limit, offset };
    
    if (filters) {
      if (filters.keyword?.trim()) {
        params.keyword = filters.keyword.trim();
      }
      if (filters.startTime) {
        params.start_time = filters.startTime;
      }
      if (filters.endTime) {
        params.end_time = filters.endTime;
      }
    }
    
    return this.api.get<QueryHistoryResponse>(`/clusters/queries/history`, params);
  }

  // Query Profile API
  getQueryProfile(queryId: string): Observable<QueryProfile> {
    return this.api.get<QueryProfile>(`/clusters/queries/${queryId}/profile`);
  }

  // Get catalogs list
  getCatalogs(): Observable<string[]> {
    return this.api.get<string[]>(`/clusters/catalogs`);
  }

  // Get databases list in a catalog
  getDatabases(catalog?: string): Observable<string[]> {
    // Always pass catalog parameter, even if empty - backend expects it
    const params = catalog ? { catalog } : {};
    return this.api.get<string[]>(`/clusters/databases`, params);
  }

  // Get tables list for a database within an optional catalog
  getTables(catalog: string | undefined, database: string): Observable<TableInfo[]> {
    const params: any = { database };
    if (catalog) {
      params.catalog = catalog;
    }
    return this.api.get<TableInfo[]>(`/clusters/tables`, params);
  }

  // Execute SQL API
  // Use extended timeout (650 seconds) for large queries to match Nginx proxy_read_timeout (600s)
  executeSQL(sql: string, limit?: number, catalog?: string, database?: string): Observable<QueryExecuteResult> {
    const request: QueryExecuteRequest = { sql, limit, catalog, database };
    return this.api.post<QueryExecuteResult>(`/clusters/queries/execute`, request, 650000);
  }

  // Profile APIs
  listProfiles(): Observable<ProfileListItem[]> {
    return this.api.get<ProfileListItem[]>(`/clusters/profiles`);
  }

  getProfile(queryId: string): Observable<ProfileDetail> {
    return this.api.get<ProfileDetail>(`/clusters/profiles/${queryId}`);
  }

  analyzeProfile(queryId: string): Observable<ProfileAnalysisResult> {
    return this.api.get<ProfileAnalysisResult>(`/clusters/profiles/${queryId}/analyze`);
  }

  /**
   * Enhance profile analysis with LLM (called async after DAG is rendered)
   * @param clusterId Cluster ID
   * @param queryId Query ID
   * @param payload Request payload containing analysis_data and force_refresh flag
   */
  enhanceProfileWithLLM(clusterId: number, queryId: string, payload: { analysis_data: any, force_refresh?: boolean }): Observable<any> {
    return this.api.post<any>(`/clusters/${clusterId}/profiles/${queryId}/enhance`, payload);
  }

  /**
   * SQL Diagnosis with LLM - analyze SQL performance issues
   * @param clusterId Cluster ID
   * @param sql SQL statement to diagnose
   * @param database Optional database name
   * @param catalog Optional catalog name
   */
  diagnoseSQL(clusterId: number, sql: string, database?: string, catalog?: string): Observable<SqlDiagResponse> {
    return this.api.post<SqlDiagResponse>(`/clusters/${clusterId}/sql/diagnose`, { sql, database, catalog });
  }
}

// SQL Diagnosis Response types
export interface SqlDiagResponse {
  ok: boolean;
  data?: SqlDiagResult;
  err?: string;
  cached: boolean;
  ms: number;
}

export interface SqlDiagResult {
  sql: string;
  changed: boolean;
  perf_issues: PerfIssue[];
  explain_analysis?: ExplainAnalysis;
  summary: string;
  confidence: number;
}

export interface PerfIssue {
  type: string;
  severity: string;
  desc: string;
  fix?: string;
}

export interface ExplainAnalysis {
  scan_type?: string;
  join_strategy?: string;
  estimated_rows?: number;
  estimated_cost?: string;
}

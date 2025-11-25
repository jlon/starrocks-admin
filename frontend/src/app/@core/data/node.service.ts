import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiService } from './api.service';

export interface Backend {
  BackendId: string;
  IP: string;  // Changed from Host to IP to match StarRocks API
  HeartbeatPort: string;
  BePort: string;
  HttpPort: string;
  BrpcPort: string;
  LastStartTime: string;
  LastHeartbeat: string;
  Alive: string;
  SystemDecommissioned: string;
  TabletNum: string;
  DataUsedCapacity: string;
  TotalCapacity: string;
  UsedPct: string;
  MaxDiskUsedPct: string;
  CpuUsedPct: string;
  MemUsedPct: string;
  NumRunningQueries: string;
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
}

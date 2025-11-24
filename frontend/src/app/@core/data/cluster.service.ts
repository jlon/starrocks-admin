import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiService } from './api.service';

export type DeploymentMode = 'shared_nothing' | 'shared_data';

export interface Cluster {
  id: number;
  name: string;
  description?: string;
  fe_host: string;
  fe_http_port: number;
  fe_query_port: number;
  username: string;
  enable_ssl: boolean;
  connection_timeout: number;
  tags: string[];
  catalog: string;
  is_active: boolean;
  organization_id?: number;
  deployment_mode: DeploymentMode;
  created_at: string;
  updated_at: string;
}

export interface CreateClusterRequest {
  name: string;
  description?: string;
  fe_host: string;
  fe_http_port?: number;
  fe_query_port?: number;
  username: string;
  password: string;
  enable_ssl?: boolean;
  connection_timeout?: number;
  tags?: string[];
  catalog?: string;
  organization_id?: number;
  deployment_mode?: DeploymentMode;
}

export interface ClusterHealth {
  status: 'healthy' | 'warning' | 'critical' | 'unknown';
  checks: HealthCheck[];
  last_check_time: string;
}

export interface HealthCheck {
  name: string;
  status: string;
  message: string;
}

@Injectable({
  providedIn: 'root',
})
export class ClusterService {
  constructor(private api: ApiService) {}

  listClusters(): Observable<Cluster[]> {
    return this.api.get<Cluster[]>('/clusters');
  }

  getCluster(id: number): Observable<Cluster> {
    return this.api.get<Cluster>(`/clusters/${id}`);
  }

  createCluster(data: CreateClusterRequest): Observable<Cluster> {
    return this.api.post<Cluster>('/clusters', data);
  }

  updateCluster(id: number, data: Partial<CreateClusterRequest>): Observable<Cluster> {
    return this.api.put<Cluster>(`/clusters/${id}`, data);
  }

  deleteCluster(id: number): Observable<any> {
    return this.api.delete(`/clusters/${id}`);
  }

  getActiveCluster(): Observable<Cluster> {
    return this.api.get<Cluster>('/clusters/active');
  }

  activateCluster(id: number): Observable<Cluster> {
    return this.api.put<Cluster>(`/clusters/${id}/activate`, {});
  }

  // Test connection for new cluster (connection validation)
  testConnection(data: {
    fe_host: string;
    fe_http_port: number;
    fe_query_port: number;
    username: string;
    password: string;
    enable_ssl?: boolean;
    catalog?: string;
  }): Observable<ClusterHealth> {
    return this.api.post<ClusterHealth>('/clusters/health/test', data);
  }

  // Get health for existing cluster
  getHealth(id: number): Observable<ClusterHealth> {
    return this.api.get<ClusterHealth>(`/clusters/${id}/health`);
  }
}


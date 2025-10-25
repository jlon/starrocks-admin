import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiService } from './api.service';

export interface ClusterOverview {
  clusterId: number;
  clusterName: string;
  timestamp: string;
  healthCards: HealthCard[];
  performanceTrends: PerformanceTrends;
  resourceTrends: ResourceTrends;
  dataStatistics: DataStatistics;
  capacityPrediction: CapacityPrediction;
}

export interface HealthCard {
  title: string;
  value: string | number;
  status: 'success' | 'warning' | 'danger' | 'info';
  trend?: number; // positive = up, negative = down
  unit?: string;
  icon?: string;
  navigateTo?: string;
}

export interface PerformanceTrends {
  qpsSeries: TimeSeriesPoint[];
  latencyP99Series: TimeSeriesPoint[];
  errorRateSeries: TimeSeriesPoint[];
}

export interface ResourceTrends {
  cpuUsageSeries: TimeSeriesPoint[];
  memoryUsageSeries: TimeSeriesPoint[];
  diskUsageSeries: TimeSeriesPoint[];
  networkTxSeries: TimeSeriesPoint[];
  networkRxSeries: TimeSeriesPoint[];
  ioReadSeries: TimeSeriesPoint[];
  ioWriteSeries: TimeSeriesPoint[];
}

export interface TimeSeriesPoint {
  timestamp: string;
  value: number;
}

export interface DataStatistics {
  databaseCount: number;
  tableCount: number;
  totalDataSizeBytes: number;
  topTablesBySize: TopTableBySize[];
  topTablesByAccess: TopTableByAccess[];
  slowQueries: SlowQuery[];
  mvTotal: number;
  mvRunning: number;
  mvFailed: number;
  mvSuccess: number;
  schemaChangeRunning: number;
  schemaChangePending: number;
  schemaChangeFinished: number;
  schemaChangeFailed: number;
  activeUsers1h: number;
  activeUsers24h: number;
}

export interface TopTableBySize {
  database: string;
  table: string;
  sizeBytes: number;
  rowCount?: number;
}

export interface TopTableByAccess {
  database: string;
  table: string;
  accessCount: number;
  lastAccess: string;
  uniqueUsers: number;
}

export interface SlowQuery {
  queryId: string;
  user: string;
  database: string;
  durationMs: number;
  scanRows: number;
  scanBytes: number;
  returnRows: number;
  cpuCostMs: number;
  memCostBytes: number;
  timestamp: string;
  state: string;
  queryPreview: string;
}

export interface CapacityPrediction {
  diskTotalBytes: number;
  diskUsedBytes: number;
  diskUsagePct: number;
  dailyGrowthBytes: number;
  daysUntilFull?: number;
  predictedFullDate?: string;
  growthTrend: string; // "increasing", "stable", "decreasing"
}

// Extended Cluster Overview (All 18 modules)
export interface ExtendedClusterOverview {
  clusterId: number;
  clusterName: string;
  timestamp: string;
  health: ClusterHealth;
  kpi: KeyPerformanceIndicators;
  resources: ResourceMetrics;
  performanceTrends: PerformanceTrends;
  resourceTrends: ResourceTrends;
  dataStats?: DataStatistics;
  mvStats: MaterializedViewStats;
  loadJobs: LoadJobStats;
  transactions: TransactionStats;
  schemaChanges: SchemaChangeStats;
  compaction: CompactionStats;
  sessions: SessionStats;
  networkIo: NetworkIOStats;
  capacity?: CapacityPrediction;
  alerts: Alert[];
}

export interface ClusterHealth {
  status: 'healthy' | 'warning' | 'critical';
  score: number; // 0-100
  beNodesOnline: number;
  beNodesTotal: number;
  feNodesOnline: number;
  feNodesTotal: number;
  compactionScore: number;
  alerts: string[];
}

export interface KeyPerformanceIndicators {
  qps: number;
  qpsTrend: number;
  p99LatencyMs: number;
  p99LatencyTrend: number;
  successRate: number;
  successRateTrend: number;
  errorRate: number;
}

export interface ResourceMetrics {
  cpuUsagePct: number;
  cpuTrend: number;
  memoryUsagePct: number;
  memoryTrend: number;
  diskUsagePct: number;
  diskTrend: number;
  compactionScore: number;
  compactionStatus: string; // "normal", "warning", "critical"
}

export interface MaterializedViewStats {
  total: number;
  running: number;
  success: number;
  failed: number;
  pending: number;
}

export interface LoadJobStats {
  running: number;
  pending: number;
  finished: number;
  failed: number;
  cancelled: number;
}

export interface TransactionStats {
  running: number;
  committed: number;
  aborted: number;
}

export interface SchemaChangeStats {
  running: number;
  pending: number;
  finished: number;
  failed: number;
  cancelled: number;
}

export interface CompactionStats {
  baseCompactionRunning: number;
  cumulativeCompactionRunning: number;
  maxScore: number;
  avgScore: number;
  beScores: BECompactionScore[];
}

export interface BECompactionScore {
  beId: number;
  beHost: string;
  score: number;
}

export interface SessionStats {
  activeUsers1h: number;
  activeUsers24h: number;
  currentConnections: number;
  runningQueries: RunningQuery[];
}

export interface RunningQuery {
  queryId: string;
  user: string;
  database: string;
  startTime: string;
  durationMs: number;
  state: string;
  queryPreview: string;
}

export interface NetworkIOStats {
  networkTxBytesPerSec: number;
  networkRxBytesPerSec: number;
  diskReadBytesPerSec: number;
  diskWriteBytesPerSec: number;
}

export interface Alert {
  level: 'critical' | 'warning' | 'info';
  category: string;
  message: string;
  timestamp: string;
  action?: string;
}

@Injectable({
  providedIn: 'root',
})
export class OverviewService {
  constructor(private api: ApiService) {}

  getClusterOverview(clusterId: number, timeRange: string = '1h'): Observable<ClusterOverview> {
    return this.api.get(`/api/clusters/${clusterId}/overview`, { time_range: timeRange });
  }

  getHealthCards(clusterId: number): Observable<HealthCard[]> {
    return this.api.get(`/api/clusters/${clusterId}/overview/health`);
  }

  getPerformanceTrends(clusterId: number, timeRange: string = '1h'): Observable<PerformanceTrends> {
    return this.api.get(`/api/clusters/${clusterId}/overview/performance`, { time_range: timeRange });
  }

  getResourceTrends(clusterId: number, timeRange: string = '1h'): Observable<ResourceTrends> {
    return this.api.get(`/api/clusters/${clusterId}/overview/resources`, { time_range: timeRange });
  }

  getDataStatistics(clusterId: number): Observable<DataStatistics> {
    return this.api.get(`/api/clusters/${clusterId}/overview/data-stats`);
  }

  getCapacityPrediction(clusterId: number): Observable<CapacityPrediction> {
    return this.api.get(`/api/clusters/${clusterId}/overview/capacity-prediction`);
  }

  getSlowQueries(
    clusterId: number,
    hours: number = 24,
    minDurationMs: number = 1000,
    limit: number = 20
  ): Observable<SlowQuery[]> {
    return this.api.get(`/api/clusters/${clusterId}/overview/slow-queries`, {
      hours,
      min_duration_ms: minDurationMs,
      limit,
    });
  }

  getExtendedClusterOverview(clusterId: number, timeRange: string = '24h'): Observable<ExtendedClusterOverview> {
    return this.api.get(`/api/clusters/${clusterId}/overview/extended`, { time_range: timeRange });
  }
}


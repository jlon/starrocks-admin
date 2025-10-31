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
  description?: string; // Tooltip description for the metric
  cardId?: string; // Unique identifier for special cards (latency, disk, etc.)
}

export interface PerformanceTrends {
  qps: TimeSeriesPoint[];
  rps: TimeSeriesPoint[];
  latency_p50: TimeSeriesPoint[];
  latency_p95: TimeSeriesPoint[];
  latency_p99: TimeSeriesPoint[];
  error_rate: TimeSeriesPoint[];
}

export interface ResourceTrends {
  cpu_usage: TimeSeriesPoint[];
  memory_usage: TimeSeriesPoint[];
  disk_usage: TimeSeriesPoint[];
  jvm_heap_usage: TimeSeriesPoint[];
  network_tx: TimeSeriesPoint[];
  network_rx: TimeSeriesPoint[];
  io_read: TimeSeriesPoint[];
  io_write: TimeSeriesPoint[];
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

export interface CapacityPrediction {
  disk_total_bytes: number;
  disk_used_bytes: number;
  disk_usage_pct: number;
  daily_growth_bytes: number;
  days_until_full?: number;
  predicted_full_date?: string;
  growth_trend: string; // "increasing", "stable", "decreasing"
  real_data_size_bytes: number; // Real data size from information_schema (stored in object storage)
}

// Extended Cluster Overview (All 18 modules)
export interface ExtendedClusterOverview {
  cluster_id: number;
  cluster_name: string;
  timestamp: string;
  health: ClusterHealth;
  kpi: KeyPerformanceIndicators;
  resources: ResourceMetrics;
  performance_trends: PerformanceTrends;
  resource_trends: ResourceTrends;
  data_stats?: DataStatistics;
  mv_stats: MaterializedViewStats;
  load_jobs: LoadJobStats;
  transactions: TransactionStats;
  schema_changes: SchemaChangeStats;
  compaction: CompactionStats;
  sessions: SessionStats;
  network_io: NetworkIOStats;
  capacity?: CapacityPrediction;
  alerts: Alert[];
}

export interface ClusterHealth {
  status: 'healthy' | 'warning' | 'critical';
  score: number; // 0-100
  starrocks_version: string; // StarRocks version
  be_nodes_online: number;
  be_nodes_total: number;
  fe_nodes_online: number;
  fe_nodes_total: number;
  compaction_score: number;
  alerts: string[];
}

export interface KeyPerformanceIndicators {
  qps: number;
  qps_trend: number;
  p99_latency_ms: number;
  p99_latency_trend: number;
  success_rate: number;
  success_rate_trend: number;
  error_rate: number;
}

export interface ResourceMetrics {
  cpu_usage_pct: number;
  cpu_trend: number;
  memory_usage_pct: number;
  memory_trend: number;
  disk_usage_pct: number;
  disk_trend: number;
  compaction_score: number;
  compaction_status: string; // "normal", "warning", "critical"
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

// Compaction Stats for Storage-Compute Separation Architecture
// Reference: https://forum.mirrorship.cn/t/topic/13256
// In shared-data mode:
// - Compaction is scheduled by FE at Partition level
// - No distinction between base/cumulative compaction
// - Score is per-partition, not per-BE
export interface CompactionStats {
  baseCompactionRunning: number;           // Always 0 in shared-data mode
  cumulativeCompactionRunning: number;     // Total compaction tasks running
  maxScore: number;                        // Max compaction score across all partitions (from FE)
  avgScore: number;                        // Same as maxScore in shared-data mode
  beScores: BECompactionScore[];           // Empty in shared-data mode
}

export interface BECompactionScore {
  beId: number;
  beHost: string;
  score: number;
}

// Compaction Detail Stats for Storage-Compute Separation Architecture
// Provides detailed compaction monitoring including:
// - Top 10 partitions by compaction score
// - Running and finished task statistics
// - Duration statistics (min, max, avg)
export interface CompactionDetailStats {
  topPartitions: TopPartitionByScore[];
  taskStats: CompactionTaskStats;
  durationStats: CompactionDurationStats;
}

export interface TopPartitionByScore {
  dbName: string;
  tableName: string;
  partitionName: string;
  maxScore: number;
  avgScore: number;
  p50Score: number;
}

export interface CompactionTaskStats {
  runningCount: number;
  finishedCount: number;
  totalCount: number;
}

export interface CompactionDurationStats {
  minDurationMs: number;
  maxDurationMs: number;
  avgDurationMs: number;
}

export interface SessionStats {
  active_users_1h: number;
  active_users_24h: number;
  current_connections: number;
  running_queries: RunningQuery[];
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

  /**
   * Format bytes to human-readable size with adaptive unit
   * 自适应单位显示：大于1024T显示P，大于1024G显示T，以此类推
   */
  formatBytes(bytes: number): { value: string; unit: string } {
    if (bytes === 0) return { value: '0', unit: 'B' };
    
    const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    const k = 1024;
    
    // Find the appropriate unit
    let unitIndex = 0;
    let value = bytes;
    
    while (value >= k && unitIndex < units.length - 1) {
      value /= k;
      unitIndex++;
    }
    
    // Format value: show 1 decimal place for values < 10, otherwise round
    const formattedValue = value < 10 ? value.toFixed(1) : Math.round(value).toString();
    
    return {
      value: formattedValue,
      unit: units[unitIndex]
    };
  }

  getClusterOverview(clusterId: number, timeRange: string = '1h'): Observable<ClusterOverview> {
    return this.api.get(`/clusters/${clusterId}/overview`, { time_range: timeRange });
  }

  getHealthCards(clusterId: number): Observable<HealthCard[]> {
    return this.api.get(`/clusters/${clusterId}/overview/health`);
  }

  getPerformanceTrends(clusterId: number, timeRange: string = '1h'): Observable<PerformanceTrends> {
    return this.api.get(`/clusters/${clusterId}/overview/performance`, { time_range: timeRange });
  }

  getResourceTrends(clusterId: number, timeRange: string = '1h'): Observable<ResourceTrends> {
    return this.api.get(`/clusters/${clusterId}/overview/resources`, { time_range: timeRange });
  }

  getDataStatistics(clusterId: number): Observable<DataStatistics> {
    return this.api.get(`/clusters/${clusterId}/overview/data-stats`);
  }

  getCapacityPrediction(clusterId: number): Observable<CapacityPrediction> {
    return this.api.get(`/clusters/${clusterId}/overview/capacity-prediction`);
  }

  getExtendedClusterOverview(timeRange: string = '24h'): Observable<ExtendedClusterOverview> {
    return this.api.get(`/clusters/overview/extended`, { time_range: timeRange });
  }

  /**
   * Get compaction detail statistics for storage-compute separation architecture
   * 
   * @param timeRange Time range for task statistics: 1h, 6h, 24h, 3d (default: 1h)
   * @returns CompactionDetailStats including:
   *   - Top 10 partitions by compaction score
   *   - Running and finished task counts
   *   - Duration statistics (min, max, avg)
   */
  getCompactionDetailStats(timeRange: string = '1h'): Observable<CompactionDetailStats> {
    return this.api.get(`/clusters/overview/compaction-details`, { time_range: timeRange });
  }

  /**
   * Transform ExtendedClusterOverview to HealthCard[] for display
   * Converts backend data structure to frontend card format
   */
  transformToHealthCards(overview: ExtendedClusterOverview): HealthCard[] {
    return [
      // ========== 核心健康指标 (P0, 7个) ==========
      // 1. StarRocks 版本
      {
        title: 'SR 版本',
        value: overview.health.starrocks_version || 'Unknown',
        unit: '',
        trend: 0,
        status: 'info',
        icon: 'info-outline',
        description: 'StarRocks集群版本号',
        cardId: 'sr_version'
      },
      // 2. BE 节点状态
      {
        title: 'BE 节点',
        value: `${overview.health.be_nodes_online}/${overview.health.be_nodes_total}`,
        unit: '',
        trend: 0,
        status: overview.health.be_nodes_online === overview.health.be_nodes_total ? 'success' : 'danger',
        icon: 'radio-outline',
        navigateTo: '/pages/starrocks/backends',
        description: 'Backend节点存活状态，负责数据存储和查询执行'
      },
      // 2. FE 节点状态
      {
        title: 'FE 节点',
        value: `${overview.health.fe_nodes_online}/${overview.health.fe_nodes_total}`,
        unit: '',
        trend: 0,
        status: overview.health.fe_nodes_online === overview.health.fe_nodes_total ? 'success' : 'danger',
        icon: 'monitor-outline',
        navigateTo: '/pages/starrocks/frontends',
        description: 'Frontend节点存活状态，负责元数据管理和SQL解析'
      },
      // 3. Compaction Score
      {
        title: 'Compaction Score',
        value: Math.round(overview.resources.compaction_score).toString(),
        unit: '',
        trend: overview.resources.compaction_score > 100 ? -5 : 0,
        status: overview.resources.compaction_score > 1000 ? 'danger' :   // 🔴 紧急
                overview.resources.compaction_score > 500 ? 'warning' :   // 🟠 严重
                overview.resources.compaction_score > 100 ? 'warning' :   // 🟡 警告
                'success',
        icon: 'layers-outline',
        navigateTo: '/pages/starrocks/system',
        description: 'Partition压缩评分 (>1000紧急 >500严重 >100警告)（点击查看详情）',
        cardId: 'compaction_score'
      },
      // 4. P99 延迟
      {
        title: 'P99 延迟',
        value: Math.round(overview.kpi.p99_latency_ms).toString(),
        unit: 'ms',
        trend: overview.kpi.p99_latency_trend || 0,
        status: overview.kpi.p99_latency_ms < 1000 ? 'success' : 
                overview.kpi.p99_latency_ms < 5000 ? 'warning' : 'danger',
        icon: 'clock-outline',
        description: '99%查询的响应时间，OLAP典型值100ms-5s',
        cardId: 'latency_percentile'
      },
      // 5. 并发查询
      {
        title: '并发查询',
        value: overview.sessions.running_queries?.length.toString() || '0',
        unit: '个',
        trend: 0,
        status: 'info',
        icon: 'activity-outline',
        navigateTo: '/pages/starrocks/queries/execution',
        description: '当前正在执行的查询数，OLAP典型值1-50'
      },
      // 6. Session连接数
      {
        title: 'Session',
        value: (overview.sessions?.current_connections || 0).toString(),
        unit: '个',
        trend: 0,
        status: 'info',
        icon: 'people-outline',
        navigateTo: '/pages/starrocks/sessions',
        description: '当前活跃的Session连接数（点击查看详情）',
        cardId: 'sessions'
      },
      // 7. 数据库/表数量
      {
        title: '数据库/表',
        value: `${(overview.data_stats as any)?.database_count || 0}/${(overview.data_stats as any)?.table_count || 0}`,
        unit: '',
        trend: 0,
        status: 'info',
        icon: 'inbox-outline',
        description: '集群中数据库和表的总数量',
        cardId: 'database_table_count'
      },
      
      // ========== 资源状态 (P0, 2个) ==========
      // 8. CPU 使用
      {
        title: 'CPU 使用',
        value: Math.round(overview.resources.cpu_usage_pct).toString(),
        unit: '%',
        trend: overview.resources.cpu_trend || 0,
        status: 'info',
        icon: 'flash-outline',
        description: '集群平均CPU使用率'
      },
      // 9. 内存使用
      {
        title: '内存使用',
        value: Math.round(overview.resources.memory_usage_pct).toString(),
        unit: '%',
        trend: overview.resources.memory_trend || 0,
        status: 'info',
        icon: 'inbox-outline',
        description: '集群平均内存使用率'
      },
      
      // ========== 节点与任务 (P1, 2个) ==========
      // 10. 导入任务
      {
        title: '导入任务',
        value: (overview.load_jobs?.running || 0).toString(),
        unit: '个',
        trend: 0,
        status: 'info',
        icon: 'upload-outline',
        navigateTo: '/pages/starrocks/system',
        description: '正在运行的数据导入任务（点击查看详情）',
        cardId: 'load_jobs'
      },
      // 11. Compaction 任务
      {
        title: 'Compaction 任务',
        value: (overview.compaction?.cumulativeCompactionRunning || 0).toString(),
        unit: '个',
        trend: 0,
        status: 'info',
        icon: 'sync-outline',
        navigateTo: '/pages/starrocks/system',
        description: '正在运行的 Compaction 任务（点击查看详情）',
        cardId: 'compactions'
      },
      // 12. 物化视图
      {
        title: '物化视图',
        value: (overview.mv_stats?.total || 0).toString(),
        unit: '个',
        trend: 0,
        status: 'success',
        icon: 'cube-outline',
        navigateTo: '/pages/starrocks/materialized-views',
        description: '物化视图总数量'
      },
      
      // ========== 数据与容量 (P1, 3个) ==========
      // 12. 缓存增量
      (() => {
        const formatted = overview.capacity 
          ? this.formatBytes(Math.abs(overview.capacity.daily_growth_bytes))
          : { value: '0', unit: 'B' };
        return {
          title: '缓存增量',
          value: formatted.value,
          unit: `${formatted.unit}/天`,
          trend: 0,
          status: 'info',
          icon: 'trending-up-outline',
          description: 'BE本地缓存数据的每日平均增长量（基于线性回归）'
        };
      })(),
      // 13. 本地磁盘/缓存使用 (switchable) - 点击切换显示使用率%或使用量TB
      {
        title: '本地磁盘',
        value: overview.capacity ? Math.round(overview.capacity.disk_usage_pct).toString() : '0',
        unit: '%',
        trend: 0,
        status: overview.capacity && overview.capacity.disk_usage_pct > 80 ? 'warning' : 'info',
        icon: 'hard-drive-outline',
        description: 'BE节点本地磁盘最大使用率（点击切换到缓存使用）',
        cardId: 'disk_cache_metric'
      },
      // 14. 真实数据
      (() => {
        const formatted = overview.capacity 
          ? this.formatBytes(overview.capacity.real_data_size_bytes)
          : { value: '0', unit: 'B' };
        return {
          title: '数据总量',
          value: formatted.value,
          unit: formatted.unit,
          trend: 0,
          status: 'success',
          icon: 'archive-outline',
          description: '对象存储中的实际数据总量（从information_schema统计）'
        };
      })()
    ];
  }

  /**
   * Transform ExtendedClusterOverview to DataStatistics
   */
  transformDataStatistics(overview: ExtendedClusterOverview): DataStatistics {
    const dataStats = overview.data_stats as any; // Use 'any' to access snake_case fields from backend
    return {
      databaseCount: dataStats?.database_count || 0,
      tableCount: dataStats?.table_count || 0,
      totalDataSizeBytes: dataStats?.total_data_size || 0,
      mvTotal: dataStats?.mv_total || 0,
      mvRunning: dataStats?.mv_running || 0,
      mvSuccess: dataStats?.mv_success || 0,
      mvFailed: dataStats?.mv_failed || 0,
      schemaChangeRunning: dataStats?.schema_change_running || 0,
      schemaChangePending: dataStats?.schema_change_pending || 0,
      schemaChangeFinished: dataStats?.schema_change_finished || 0,
      schemaChangeFailed: dataStats?.schema_change_failed || 0,
      activeUsers1h: dataStats?.active_users_1h || 0,
      activeUsers24h: dataStats?.active_users_24h || 0,
      topTablesBySize: (dataStats?.top_tables_by_size || []).map((t: any) => ({
        database: t.database,
        table: t.table,
        sizeBytes: t.size_bytes,
        rowCount: t.rows
      })),
      topTablesByAccess: (dataStats?.top_tables_by_access || []).map((t: any) => ({
        database: t.database,
        table: t.table,
        accessCount: t.access_count,
        lastAccess: t.last_access
      }))
    };
  }
}


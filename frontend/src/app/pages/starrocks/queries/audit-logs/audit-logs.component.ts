import { Component, OnInit, OnDestroy, TemplateRef, ViewChild } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { NbToastrService, NbDialogService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { NodeService, QueryHistoryItem } from '../../../../@core/data/node.service';
import { ClusterContextService } from '../../../../@core/data/cluster-context.service';
import { Cluster } from '../../../../@core/data/cluster.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { MetricThresholds, renderMetricBadge } from '../../../../@core/utils/metric-badge';
import { renderLongText } from '../../../../@core/utils/text-truncate';
import { AuthService } from '../../../../@core/data/auth.service';

@Component({
  selector: 'ngx-audit-logs',
  templateUrl: './audit-logs.component.html',
  styleUrls: ['./audit-logs.component.scss'],
})
export class AuditLogsComponent implements OnInit, OnDestroy {
  // Data sources
  historySource: LocalDataSource = new LocalDataSource();
  
  // Expose Math to template
  Math = Math;
  
  // State
  clusterId: number;
  activeCluster: Cluster | null = null;
  loading = true;
  autoRefresh = false; // Default: disabled
  refreshInterval: any;
  selectedRefreshInterval: number | 'off' = 'off'; // Default: off (Grafana style)
  refreshIntervalOptions = [
    { value: 'off', label: '关闭' },
    { value: 3, label: '3秒' },
    { value: 5, label: '5秒' },
    { value: 10, label: '10秒' },
    { value: 30, label: '30秒' },
    { value: 60, label: '1分钟' },
  ];
  private destroy$ = new Subject<void>();
  private readonly durationThresholds: MetricThresholds = { warn: 3000, danger: 10000 };

  // Profile dialog
  currentProfile: any = null;
  @ViewChild('profileDialog') profileDialogTemplate: TemplateRef<any>;

  // History search filters
  searchKeyword: string = '';
  searchStartTime: string = '';
  searchEndTime: string = '';

  // Pagination state for history
  historyPageSize: number = 10;
  historyCurrentPage: number = 1;
  historyTotalCount: number = 0;

  // History queries settings with Profile button
  historySettings = {
    mode: 'external',
    hideSubHeader: false, // Enable search
    noDataMessage: '暂无审计日志记录',
    actions: {
      add: false,
      edit: true,
      delete: false,
      position: 'right',
      width: '80px',
    },
    edit: {
      editButtonContent: '<i class="nb-search"></i>',
    },
    pager: {
      display: false, // Disable ng2-smart-table's built-in pagination (we'll use custom pagination)
    },
    columns: {
      query_id: { title: 'Query ID', type: 'string' },
      user: { title: '用户', type: 'string', width: '8%' },
      default_db: { title: '数据库', type: 'string', width: '8%' },
      query_type: { title: '类型', type: 'string', width: '8%' },
      query_state: { title: '状态', type: 'string', width: '8%' },
      start_time: { title: '开始时间', type: 'string', width: '12%' },
      total_ms: {
        title: '耗时(ms)',
        type: 'html',
        width: '8%',
        valuePrepareFunction: (value: string | number) => renderMetricBadge(value, this.durationThresholds),
      },
      sql_statement: { 
        title: 'SQL', 
        type: 'html',
        valuePrepareFunction: (value: any) => renderLongText(value, 100),
      },
    },
  };

  constructor(
    private nodeService: NodeService,
    private route: ActivatedRoute,
    private toastrService: NbToastrService,
    private clusterContext: ClusterContextService,
    private dialogService: NbDialogService,
    private authService: AuthService,
  ) {
    // Try to get clusterId from route first (for direct navigation)
    const routeClusterId = parseInt(this.route.snapshot.paramMap.get('clusterId') || '0', 10);
    this.clusterId = routeClusterId;
  }

  ngOnInit(): void {
    // Subscribe to active cluster changes
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
        if (cluster) {
          // Always use the active cluster (override route parameter)
          const newClusterId = cluster.id;
          if (this.clusterId !== newClusterId) {
            this.clusterId = newClusterId;
            // Reset pagination when cluster changes
            this.historyCurrentPage = 1;
            this.loadHistoryQueries();
          }
        }
        // Backend will handle "no active cluster" case
      });

    // Load data - backend will get active cluster automatically
    this.loadHistoryQueries();
  }

  ngOnDestroy(): void {
    this.stopAutoRefresh();
    this.destroy$.next();
    this.destroy$.complete();
  }

  // Grafana-style: selecting an interval automatically enables auto-refresh
  // Selecting 'off' disables auto-refresh
  onRefreshIntervalChange(interval: number | 'off'): void {
    this.selectedRefreshInterval = interval;
    
    if (interval === 'off') {
      // Disable auto-refresh
      this.autoRefresh = false;
      this.stopAutoRefresh();
    } else {
      // Enable auto-refresh with selected interval
      this.autoRefresh = true;
      this.stopAutoRefresh();
      this.startAutoRefresh();
    }
  }

  startAutoRefresh(): void {
    this.stopAutoRefresh(); // Clear any existing interval
    
    // Only start if interval is a number (not 'off')
    if (typeof this.selectedRefreshInterval !== 'number') {
      return;
    }
    
    this.refreshInterval = setInterval(() => {
      // Stop auto-refresh if user is not authenticated (logged out)
      if (!this.authService.isAuthenticated()) {
        this.autoRefresh = false;
        this.selectedRefreshInterval = 'off';
        this.stopAutoRefresh();
        return;
      }
      // Only update data, don't show loading spinner during auto-refresh
      this.loadHistoryQueriesSilently();
    }, this.selectedRefreshInterval * 1000);
  }

  stopAutoRefresh(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  // Load query history with pagination and filters
  loadHistoryQueries(): void {
    this.loading = true;
    
    // Prepare filters
    const filters = {
      keyword: this.searchKeyword?.trim() || undefined,
      startTime: this.searchStartTime || undefined,
      endTime: this.searchEndTime || undefined,
    };
    
    this.nodeService
      .listQueryHistory(
        this.historyPageSize, 
        (this.historyCurrentPage - 1) * this.historyPageSize,
        filters
      )
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (data) => {
          this.historySource.load(data.data);
          this.historyTotalCount = data.total;
          this.loading = false;
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.handleClusterError(error),
            '加载失败'
          );
          this.historySource.load([]);
          this.loading = false;
        },
      });
  }

  // Load query history silently (for auto-refresh, no loading spinner)
  loadHistoryQueriesSilently(): void {
    // Prepare filters
    const filters = {
      keyword: this.searchKeyword?.trim() || undefined,
      startTime: this.searchStartTime || undefined,
      endTime: this.searchEndTime || undefined,
    };
    
    // Only update data, don't show loading spinner during auto-refresh
    this.nodeService
      .listQueryHistory(
        this.historyPageSize, 
        (this.historyCurrentPage - 1) * this.historyPageSize,
        filters
      )
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (data) => {
          this.historySource.load(data.data);
          this.historyTotalCount = data.total;
        },
        error: (error) => {
          // Silently handle errors during auto-refresh, don't show toast
          console.error('[AuditLogs] Auto-refresh error:', error);
        },
      });
  }

  // Calculate total pages
  get historyTotalPages(): number {
    return Math.ceil(this.historyTotalCount / this.historyPageSize);
  }

  // Handle page change
  onHistoryPageChange(page: number): void {
    if (page < 1 || page > this.historyTotalPages) {
      return;
    }
    this.historyCurrentPage = page;
    this.loadHistoryQueries();
  }

  // Handle page size change
  onHistoryPageSizeChange(size: number): void {
    this.historyPageSize = size;
    this.historyCurrentPage = 1; // Reset to first page
    this.loadHistoryQueries();
  }

  // Handle edit action (View Profile)
  onEditProfile(event: any): void {
    const query: QueryHistoryItem = event.data;
    this.viewProfile(query.query_id);
  }

  // View query profile
  viewProfile(queryId: string): void {
    this.nodeService.getQueryProfile(queryId).subscribe({
      next: (profile) => {
        this.currentProfile = profile;
        // Open profile dialog
        this.dialogService.open(this.profileDialogTemplate, {
          context: { profile },
        });
      },
      error: (error) => {
        this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '加载失败');
      },
    });
  }

  // Search history methods
  searchHistory(): void {
    this.loadHistoryQueries();
  }

  // Check if there are active filters
  hasActiveFilters(): boolean {
    return !!(this.searchKeyword?.trim() || this.searchStartTime || this.searchEndTime);
  }

  // Clear all filters
  clearFilters(): void {
    this.searchKeyword = '';
    this.searchStartTime = '';
    this.searchEndTime = '';
    this.searchHistory();
  }

  // Note: Filtering is now handled by the backend API
  // This method is kept for reference but no longer used
  applyHistoryFilters(queries: QueryHistoryItem[]): QueryHistoryItem[] {
    // Backend handles filtering now, so this method is not used
    return queries;
  }
}

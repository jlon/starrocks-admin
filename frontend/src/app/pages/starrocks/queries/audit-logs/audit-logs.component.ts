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
  selectedRefreshInterval = 5; // Default 5 seconds
  refreshIntervalOptions = [
    { value: 3, label: '3秒' },
    { value: 5, label: '5秒' },
    { value: 10, label: '10秒' },
    { value: 30, label: '30秒' },
    { value: 60, label: '1分钟' },
  ];
  private destroy$ = new Subject<void>();

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
      total_ms: { title: '耗时(ms)', type: 'number', width: '8%' },
      sql_statement: { title: 'SQL', type: 'string' },
    },
  };

  constructor(
    private nodeService: NodeService,
    private route: ActivatedRoute,
    private toastrService: NbToastrService,
    private clusterContext: ClusterContextService,
    private dialogService: NbDialogService,
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
        } else {
          // No active cluster - show error and stop loading
          this.loading = false;
          this.toastrService.danger(
            '请先激活一个集群',
            '未选择集群'
          );
        }
      });

    // Load queries if clusterId is already set from route
    if (this.clusterId && this.clusterId > 0) {
      this.loadHistoryQueries();
    }
  }

  ngOnDestroy(): void {
    this.stopAutoRefresh();
    this.destroy$.next();
    this.destroy$.complete();
  }

  // Auto refresh methods
  toggleAutoRefresh(): void {
    this.autoRefresh = !this.autoRefresh;
    if (this.autoRefresh) {
      this.startAutoRefresh();
    } else {
      this.stopAutoRefresh();
    }
  }

  onRefreshIntervalChange(interval: number): void {
    this.selectedRefreshInterval = interval;
    if (this.autoRefresh) {
      // Restart with new interval
      this.stopAutoRefresh();
      this.startAutoRefresh();
    }
  }

  startAutoRefresh(): void {
    this.stopAutoRefresh(); // Clear any existing interval
    this.refreshInterval = setInterval(() => {
      this.loadHistoryQueries();
    }, this.selectedRefreshInterval * 1000);
  }

  stopAutoRefresh(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  // Load query history with pagination
  loadHistoryQueries(): void {
    if (!this.clusterId || this.clusterId === 0) {
      this.loading = false;
      return;
    }

    this.loading = true;
    this.nodeService
      .listQueryHistory(this.historyPageSize, (this.historyCurrentPage - 1) * this.historyPageSize)
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
    if (!this.clusterId) {
      return;
    }
    
    // Reload history with current filters
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

  applyHistoryFilters(queries: QueryHistoryItem[]): QueryHistoryItem[] {
    let filtered = queries;

    // Filter by keyword (search in query_id and sql_statement)
    if (this.searchKeyword && this.searchKeyword.trim() !== '') {
      const keyword = this.searchKeyword.toLowerCase();
      filtered = filtered.filter(q => 
        q.query_id.toLowerCase().includes(keyword) || 
        q.sql_statement.toLowerCase().includes(keyword) ||
        q.user.toLowerCase().includes(keyword)
      );
    }

    // Filter by start time
    if (this.searchStartTime) {
      filtered = filtered.filter(q => q.start_time >= this.searchStartTime);
    }

    // Filter by end time
    if (this.searchEndTime) {
      filtered = filtered.filter(q => q.start_time <= this.searchEndTime);
    }

    return filtered;
  }
}

import { Component, OnInit, OnDestroy } from '@angular/core';

import { NbToastrService, NbDialogService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { Cluster } from '../../../@core/data/cluster.service';
import { NodeService, Session } from '../../../@core/data/node.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';
import { MetricThresholds, renderMetricBadge } from '../../../@core/utils/metric-badge';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';

@Component({
  selector: 'ngx-sessions',
  templateUrl: './sessions.component.html',
  styleUrls: ['./sessions.component.scss'],
})
export class SessionsComponent implements OnInit, OnDestroy {
  clusterId: number;
  activeCluster: Cluster | null = null;
  sessions: Session[] = [];
  source: LocalDataSource = new LocalDataSource();
  loading = false;
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
  private readonly sessionDurationThresholds: MetricThresholds = { warn: 60, danger: 300 };

  settings = {
    hideSubHeader: false, // Enable search
    noDataMessage: '当前没有活动会话',
    actions: {
      add: false,
      edit: false,
      delete: true,
      position: 'right',
    },
    delete: {
      deleteButtonContent: '<i class="nb-trash"></i>',
      confirmDelete: true,
    },
    pager: {
      display: true,
      perPage: 15,
    },
    columns: {
      id: {
        title: 'Session ID',
        type: 'string',
        width: '10%',
      },
      user: {
        title: 'User',
        type: 'string',
        width: '10%',
      },
      host: {
        title: 'Host',
        type: 'string',
        width: '15%',
      },
      db: {
        title: 'Database',
        type: 'string',
        width: '10%',
        valuePrepareFunction: (value: any) => value || 'N/A',
      },
      command: {
        title: 'Command',
        type: 'string',
        width: '10%',
      },
      time: {
        title: 'Time (s)',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string | number) => renderMetricBadge(value, this.sessionDurationThresholds),
      },
      state: {
        title: 'State',
        type: 'string',
        width: '10%',
      },
      info: {
        title: 'Info',
        type: 'string',
        width: '25%',
        valuePrepareFunction: (value: any) => {
          if (!value) return 'N/A';
          return value.length > 100 ? value.substring(0, 100) + '...' : value;
        },
      },
    },
  };

  constructor(
    
    private toastrService: NbToastrService,
    private dialogService: NbDialogService,
    private confirmDialogService: ConfirmDialogService,
    private clusterContext: ClusterContextService,
    private nodeService: NodeService,
  ) {
    // Try to get clusterId from route first
    // Get clusterId from ClusterContextService
    this.clusterId = this.clusterContext.getActiveClusterId() || 0;
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
            this.loadSessions();
          }
        }
        // Backend will handle "no active cluster" case
      });

    // Load data - backend will get active cluster automatically
    this.loadSessions();
    if (this.autoRefresh) {
      this.startAutoRefresh();
    }
  }

  ngOnDestroy(): void {
    this.stopAutoRefresh();
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadSessions(): void {
    // Backend will get active cluster automatically - no need to check clusterId
    this.loading = true;
    this.nodeService.getSessions().subscribe({
      next: (sessions) => {
        this.sessions = sessions;
        this.source.load(sessions);
        this.loading = false;
      },
      error: (error) => {
        console.error('[Sessions] Error loading sessions:', error);
        this.toastrService.danger(
          ErrorHandler.handleClusterError(error),
          '错误'
        );
        this.sessions = [];
        this.source.load([]);
        this.loading = false;
      },
    });
  }

  onDelete(event: any): void {
    this.killSession(event.data);
  }

  killSession(session: Session): void {
    this.confirmDialogService.confirm(
      '确认终止会话',
      `确定要终止会话 ${session.id} 吗？`,
      '终止',
      '取消',
      'danger'
    ).subscribe(confirmed => {
      if (confirmed) {
        this.loading = true;
        this.nodeService.killSession(session.id).subscribe({
          next: () => {
            this.toastrService.success(`会话 ${session.id} 已成功终止`, '成功');
            this.loadSessions();
          },
          error: (error) => {
            this.toastrService.danger(
              error.error?.message || '终止会话失败',
              '错误'
            );
            this.loading = false;
          },
        });
      }
    });
  }

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
      this.loadSessions();
    }, this.selectedRefreshInterval * 1000);
  }

  stopAutoRefresh(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  refresh(): void {
    this.loadSessions();
  }
}


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
import { renderLongText } from '../../../@core/utils/text-truncate';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';
import { AuthService } from '../../../@core/data/auth.service';

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
  // Session duration thresholds: 1min(60s)=warn, 5min(300s)=danger
  private readonly sessionDurationThresholds: MetricThresholds = { warn: 60, danger: 300 };
  
  // Filter state for sessions
  sessionFilter: {
    sleepOnly?: boolean;
    slowOnly?: boolean;
  } = {};

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
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string, row: Session) => this.renderCommandBadge(value, row),
      },
      time: {
        title: 'Time (s)',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string | number) => renderMetricBadge(value, this.sessionDurationThresholds),
      },
      state: {
        title: 'State',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string) => this.renderStateBadge(value),
      },
      info: {
        title: 'Info',
        type: 'html',
        width: '25%',
        valuePrepareFunction: (value: any) => {
          if (!value) return 'N/A';
          return renderLongText(value, 80);
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
    private authService: AuthService,
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
      next: (allSessions) => {
        this.updateSessionsData(allSessions);
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

  // Load sessions silently (for auto-refresh, no loading spinner)
  loadSessionsSilently(): void {
    // Only update data, don't show loading spinner during auto-refresh
    this.nodeService.getSessions().subscribe({
      next: (allSessions) => {
        this.updateSessionsData(allSessions);
      },
      error: (error) => {
        // Silently handle errors during auto-refresh, don't show toast
        console.error('[Sessions] Auto-refresh error:', error);
      },
    });
  }

  // Update sessions data (shared logic)
  private updateSessionsData(allSessions: Session[]): void {
    // Apply filters
    let filteredSessions = allSessions;
    
    if (this.sessionFilter.sleepOnly) {
      filteredSessions = filteredSessions.filter(s => {
        const cmdLower = s.command?.toLowerCase() || '';
        const stateLower = s.state?.toLowerCase() || '';
        return cmdLower === 'sleep' || 
               stateLower.includes('sleep') ||
               cmdLower === 'daemon';
      });
    }
    
    if (this.sessionFilter.slowOnly) {
      filteredSessions = filteredSessions.filter(s => {
        const time = this.parseTime(s.time);
        return time >= 60; // 1 minute
      });
    }
    
    this.sessions = filteredSessions;
    this.source.load(filteredSessions);
  }

  onDeleteConfirm(event: any): void {
    const session = event.data as Session;

    this.confirmDialogService.confirm(
      '确认终止会话',
      `确定要终止会话 ${session.id} 吗？`,
      '终止',
      '取消',
      'danger'
    ).subscribe(confirmed => {
      if (!confirmed) {
        event.confirm.reject();
        return;
      }

      this.loading = true;
      this.nodeService.killSession(session.id).subscribe({
        next: () => {
          this.toastrService.success(`会话 ${session.id} 已成功终止`, '成功');
          event.confirm.resolve();
          this.loadSessions();
        },
        error: (error) => {
          this.toastrService.danger(
            error.error?.message || '终止会话失败',
            '错误'
          );
          event.confirm.reject();
          this.loading = false;
        },
      });
    });
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
      this.loadSessionsSilently();
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

  // Render command badge
  renderCommandBadge(value: string, row: Session): string {
    const cmd = (value || '').toLowerCase();
    if (cmd === 'sleep') {
      return '<span class="badge badge-secondary">Sleep</span>';
    } else if (cmd === 'query') {
      return '<span class="badge badge-primary">Query</span>';
    } else if (cmd === 'connect') {
      return '<span class="badge badge-info">Connect</span>';
    }
    return value || 'N/A';
  }

  // Render state badge
  renderStateBadge(value: string): string {
    if (!value || value.trim() === '') {
      return '<span class="text-hint">-</span>';
    }
    const state = value.toLowerCase();
    if (state.includes('sleep')) {
      return '<span class="badge badge-secondary">Sleep</span>';
    } else if (state.includes('query')) {
      return '<span class="badge badge-primary">Query</span>';
    }
    return value;
  }

  // Parse time from string
  parseTime(value: string | number): number {
    if (typeof value === 'number') {
      return value;
    }
    const num = parseFloat(value.toString().replace(/[^0-9.-]/g, ''));
    return isNaN(num) ? 0 : num;
  }

  // Apply filter
  applySessionFilter(): void {
    this.loadSessions();
  }

  // Reset filter
  resetSessionFilter(): void {
    this.sessionFilter = {};
    this.loadSessions();
  }

  // Clear sleeping connections
  clearSleepingConnections(): void {
    // Get all sessions first (not filtered)
    this.nodeService.getSessions().subscribe({
      next: (allSessions) => {
        // Fix: StarRocks returns Command as "Sleep" (capitalized) or "Daemon"
        // We should check both command and info fields
        const sleepingSessions = allSessions.filter(s => {
          const cmdLower = s.command?.toLowerCase() || '';
          const stateLower = s.state?.toLowerCase() || '';
          
          // Match sleep connections: command is "Sleep" or state contains "sleep"
          return cmdLower === 'sleep' || 
                 stateLower.includes('sleep') ||
                 cmdLower === 'daemon'; // Daemon connections are also idle
        });

        if (sleepingSessions.length === 0) {
          this.toastrService.info('当前没有睡眠连接', '提示');
          return;
        }

        this.confirmDialogService.confirm(
          '确认清除睡眠连接',
          `确定要清除 ${sleepingSessions.length} 个睡眠连接吗？`,
          '清除',
          '取消',
          'warning'
        ).subscribe(confirmed => {
          if (!confirmed) {
            return;
          }

          this.loading = true;
          let successCount = 0;
          let failCount = 0;
          let completed = 0;

          sleepingSessions.forEach(session => {
            this.nodeService.killSession(session.id).subscribe({
              next: () => {
                successCount++;
                completed++;
                if (completed === sleepingSessions.length) {
                  this.loading = false;
                  if (failCount === 0) {
                    this.toastrService.success(`成功清除 ${successCount} 个睡眠连接`, '成功');
                  } else {
                    this.toastrService.warning(`成功清除 ${successCount} 个，失败 ${failCount} 个`, '部分成功');
                  }
                  this.loadSessions();
                }
              },
              error: (error) => {
                failCount++;
                completed++;
                if (completed === sleepingSessions.length) {
                  this.loading = false;
                  if (successCount > 0) {
                    this.toastrService.warning(`成功清除 ${successCount} 个，失败 ${failCount} 个`, '部分成功');
                  } else {
                    this.toastrService.danger('清除睡眠连接失败', '错误');
                  }
                  this.loadSessions();
                }
              },
            });
          });
        });
      },
      error: (error) => {
        this.toastrService.danger(
          ErrorHandler.handleClusterError(error),
          '获取会话列表失败'
        );
      },
    });
  }

  // Batch kill all displayed sessions
  batchKillAllSessions(): void {
    if (this.sessions.length === 0) {
      this.toastrService.warning('当前没有可查杀的会话', '提示');
      return;
    }

    this.confirmDialogService.confirm(
      '确认批量查杀',
      `确定要查杀当前显示的 ${this.sessions.length} 个会话吗？`,
      '查杀',
      '取消',
      'danger'
    ).subscribe(confirmed => {
      if (!confirmed) {
        return;
      }

      this.loading = true;
      let successCount = 0;
      let failCount = 0;
      let completed = 0;

      this.sessions.forEach(session => {
        this.nodeService.killSession(session.id).subscribe({
          next: () => {
            successCount++;
            completed++;
            if (completed === this.sessions.length) {
              this.loading = false;
              if (failCount === 0) {
                this.toastrService.success(`成功查杀 ${successCount} 个会话`, '成功');
              } else {
                this.toastrService.warning(`成功查杀 ${successCount} 个，失败 ${failCount} 个`, '部分成功');
              }
              this.loadSessions();
            }
          },
          error: (error) => {
            failCount++;
            completed++;
            if (completed === this.sessions.length) {
              this.loading = false;
              if (successCount > 0) {
                this.toastrService.warning(`成功查杀 ${successCount} 个，失败 ${failCount} 个`, '部分成功');
              } else {
                this.toastrService.danger('批量查杀失败', '错误');
              }
              this.loadSessions();
            }
          },
        });
      });
    });
  }
}


import { Component, OnInit, OnDestroy } from '@angular/core';
import { interval, Subject } from 'rxjs';
import { takeUntil, switchMap } from 'rxjs/operators';
import { NbToastrService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { NodeService, Backend } from '../../../@core/data/node.service';
import { ClusterService, Cluster } from '../../../@core/data/cluster.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';

@Component({
  selector: 'ngx-backends',
  templateUrl: './backends.component.html',
  styleUrls: ['./backends.component.scss'],
})
export class BackendsComponent implements OnInit, OnDestroy {
  source: LocalDataSource = new LocalDataSource();
  clusterId: number;
  activeCluster: Cluster | null = null;
  clusterName: string = '';
  loading = true;
  autoRefresh = true;
  private destroy$ = new Subject<void>();

  settings = {
    mode: 'external',
    hideSubHeader: false, // Enable search
    noDataMessage: '暂无Backend节点数据',
    actions: {
      columnTitle: '操作',
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
      BackendId: {
        title: 'BE ID',
        type: 'string',
        width: '8%',
      },
      IP: {
        title: '主机',
        type: 'string',
      },
      HeartbeatPort: {
        title: '心跳端口',
        type: 'string',
        width: '10%',
      },
      BePort: {
        title: 'BE 端口',
        type: 'string',
        width: '10%',
      },
      Alive: {
        title: '状态',
        type: 'html',
        width: '8%',
        valuePrepareFunction: (value: string) => {
          const status = value === 'true' ? 'success' : 'danger';
          const text = value === 'true' ? '在线' : '离线';
          return `<span class="badge badge-${status}">${text}</span>`;
        },
      },
      TabletNum: {
        title: 'Tablet 数',
        type: 'string',
        width: '10%',
      },
      DataUsedCapacity: {
        title: '已用存储',
        type: 'string',
      },
      TotalCapacity: {
        title: '总存储',
        type: 'string',
      },
      UsedPct: {
        title: '磁盘使用率',
        type: 'string',
        width: '10%',
      },
      CpuUsedPct: {
        title: 'CPU 使用率',
        type: 'string',
        width: '12%',
      },
      MemUsedPct: {
        title: '内存使用率',
        type: 'string',
        width: '10%',
      },
      NumRunningQueries: {
        title: '运行查询数',
        type: 'string',
        width: '10%',
      },
    },
  };

  onDelete(event: any): void {
    const backend = event.data;
    const itemName = `${backend.IP}:${backend.HeartbeatPort}`;
    const additionalWarning = `⚠️ 警告: 删除节点是危险操作，请确保：\n1. 节点数据已迁移完成\n2. 集群有足够的副本数\n3. 该节点已停止服务`;
    
    this.confirmDialogService.confirmDelete(itemName, additionalWarning)
      .subscribe(confirmed => {
        if (confirmed) {
          this.nodeService.deleteBackend(backend.IP, backend.HeartbeatPort)
            .subscribe({
              next: () => {
                this.toastrService.success(
                  `Backend 节点 ${itemName} 已删除`,
                  '成功'
                );
                this.loadBackends();
              },
              error: (error) => {
                this.toastrService.danger(
                  ErrorHandler.extractErrorMessage(error),
                  '删除失败',
                );
              },
            });
        }
      });
  }

  constructor(
    private nodeService: NodeService,
    private clusterService: ClusterService,
    private clusterContext: ClusterContextService,
    private toastrService: NbToastrService,
    private confirmDialogService: ConfirmDialogService,
  ) {
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
          const newClusterId = cluster.id;
          if (this.clusterId !== newClusterId) {
            this.clusterId = newClusterId;
            this.loadClusterInfo();
            this.loadBackends();
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

    // Load data if clusterId is already set
    if (this.clusterId && this.clusterId > 0) {
      this.loadClusterInfo();
      this.loadBackends();
    } else {
      // No clusterId set - show error
      this.loading = false;
      this.toastrService.danger(
        '请先激活一个集群',
        '未选择集群'
      );
    }
    
    // Auto refresh every 10 seconds
    interval(10000)
      .pipe(
        takeUntil(this.destroy$),
        switchMap(() => this.nodeService.listBackends()),
      )
      .subscribe({
        next: (backends) => {
          if (this.autoRefresh) {
            this.source.load(backends);
          }
        },
        error: (error) => console.error('Auto refresh error:', error),
      });
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadClusterInfo(): void {
    this.clusterService.getCluster(this.clusterId).subscribe({
      next: (cluster) => {
        this.clusterName = cluster.name;
      },
      error: (error) => {
        console.error('Load cluster error:', error);
      },
    });
  }

  loadBackends(): void {
    this.loading = true;
    this.nodeService.listBackends().subscribe({
      next: (backends) => {
        this.source.load(backends);
        this.loading = false;
      },
      error: (error) => {
        this.toastrService.danger(
          ErrorHandler.extractErrorMessage(error),
          '错误',
        );
        this.loading = false;
      },
    });
  }

  toggleAutoRefresh(): void {
    this.autoRefresh = !this.autoRefresh;
    if (this.autoRefresh) {
      this.toastrService.info('已启用自动刷新', '提示');
      this.loadBackends();
    } else {
      this.toastrService.info('已禁用自动刷新', '提示');
    }
  }
}
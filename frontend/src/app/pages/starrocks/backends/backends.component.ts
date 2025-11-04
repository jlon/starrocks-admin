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
import { MetricThresholds, renderMetricBadge } from '../../../@core/utils/metric-badge';

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
  private destroy$ = new Subject<void>();
  private readonly diskThresholds: MetricThresholds = { warn: 70, danger: 85 };
  private readonly cpuThresholds: MetricThresholds = { warn: 60, danger: 85 };
  private readonly memoryThresholds: MetricThresholds = { warn: 65, danger: 85 };

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
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string) => renderMetricBadge(value, this.diskThresholds),
      },
      CpuUsedPct: {
        title: 'CPU 使用率',
        type: 'html',
        width: '12%',
        valuePrepareFunction: (value: string) => renderMetricBadge(value, this.cpuThresholds),
      },
      MemUsedPct: {
        title: '内存使用率',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string) => renderMetricBadge(value, this.memoryThresholds),
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
        }
        // Backend will handle "no active cluster" case
      });

    // Load data - backend will get active cluster automatically
    this.loadClusterInfo();
    this.loadBackends();
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
          ErrorHandler.handleClusterError(error),
          '错误',
        );
        this.loading = false;
      },
    });
  }
}
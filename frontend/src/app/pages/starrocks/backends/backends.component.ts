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
  deploymentMode: string = '';
  pageTitle: string = 'Backend 节点';
  deploymentModeText: string = '';
  deploymentModeBadgeClass: string = '';
  loading = true;
  private destroy$ = new Subject<void>();
  private readonly diskThresholds: MetricThresholds = { warn: 70, danger: 85 };
  private readonly cpuThresholds: MetricThresholds = { warn: 60, danger: 85 };
  private readonly memoryThresholds: MetricThresholds = { warn: 65, danger: 85 };

  settings = {
    mode: 'external',
    hideSubHeader: false, // Enable search
    noDataMessage: '暂无计算节点数据',
    actions: {
      columnTitle: '操作',
      add: false,
      edit: false,
      delete: true,
      position: 'right',
    },
    delete: {
      deleteButtonContent: '<i class="nb-trash"></i>',
      confirmDelete: true,  // Enable custom confirmation via deleteConfirm event
    },
    pager: {
      display: true,
      perPage: 15,
    },
    columns: {
      BackendId: {
        title: '节点 ID',
        type: 'string',
        width: '8%',
      },
      IP: {
        title: '主机',
        type: 'string',
        width: '12%',
      },
      HeartbeatPort: {
        title: '心跳端口',
        type: 'string',
        width: '8%',
      },
      BePort: {
        title: '服务端口',
        type: 'string',
        width: '8%',
      },
      HttpPort: {
        title: 'HTTP端口',
        type: 'string',
        width: '8%',
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
      Version: {
        title: '版本',
        type: 'string',
        width: '10%',
      },
      TabletNum: {
        title: 'Tablet 数',
        type: 'string',
        width: '8%',
      },
      DataUsedCapacity: {
        title: '已用存储',
        type: 'string',
        width: '10%',
      },
      TotalCapacity: {
        title: '总存储',
        type: 'string',
        width: '10%',
      },
      UsedPct: {
        title: '磁盘使用率',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string) => renderMetricBadge(value, this.diskThresholds),
      },
      CpuCores: {
        title: 'CPU核数',
        type: 'string',
        width: '8%',
      },
      CpuUsedPct: {
        title: 'CPU 使用率',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string) => renderMetricBadge(value, this.cpuThresholds),
      },
      MemLimit: {
        title: '内存限制',
        type: 'string',
        width: '10%',
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
        width: '8%',
      },
      LastHeartbeat: {
        title: '最后心跳',
        type: 'string',
        width: '12%',
      },
    },
  };

  onDeleteConfirm(event: any): void {
    const backend = event.data;
    const itemName = `${backend.IP}:${backend.HeartbeatPort}`;
    const nodeType = this.deploymentMode === 'shared_data' ? 'CN (Compute Node)' : 'BE (Backend)';
    const additionalWarning = `⚠️ 警告: 删除${nodeType}节点是危险操作，请确保：\n1. 节点数据已迁移完成\n2. 集群有足够的副本数\n3. 该节点已停止服务`;
    
    this.confirmDialogService.confirmDelete(itemName, additionalWarning)
      .subscribe(confirmed => {
        if (!confirmed) {
          event.confirm.reject();
          return;
        }

        this.nodeService.deleteBackend(backend.IP, backend.HeartbeatPort)
          .subscribe({
            next: () => {
              this.toastrService.success(
                `${nodeType} 节点 ${itemName} 已删除`,
                '成功'
              );
              event.confirm.resolve();
              this.loadBackends();
            },
            error: (error) => {
              this.toastrService.danger(
                ErrorHandler.extractErrorMessage(error),
                '删除失败',
              );
              event.confirm.reject();
            },
          });
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
    // activeCluster$ is a BehaviorSubject, so it emits immediately on subscribe
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
        if (cluster) {
          // Always use the active cluster (override route parameter)
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
    // This ensures data loads even if activeCluster$ hasn't emitted yet
    this.loadClusterInfo();
    this.loadBackends();
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadClusterInfo(): void {
    this.clusterService.getCluster(this.clusterId)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (cluster) => {
          this.clusterName = cluster.name;
          this.deploymentMode = cluster.deployment_mode || 'shared_nothing';
          
          // Update page title and badge based on deployment mode
          if (this.deploymentMode === 'shared_data') {
            this.pageTitle = 'Compute Nodes (CN)';
            this.deploymentModeText = '存算分离';
            this.deploymentModeBadgeClass = 'badge-info';
          } else {
            this.pageTitle = 'Backend Nodes (BE)';
            this.deploymentModeText = '存算一体';
            this.deploymentModeBadgeClass = 'badge-success';
          }
        },
      });
  }

  loadBackends(): void {
    this.loading = true;
    
    this.nodeService.listBackends()
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (backends) => {
          this.source.load(backends);
          this.loading = false;
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.handleClusterError(error),
            '错误',
          );
          this.source.load([]);
          this.loading = false;
        },
      });
  }
}
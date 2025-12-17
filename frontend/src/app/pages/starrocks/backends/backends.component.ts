import { Component, OnInit, OnDestroy } from '@angular/core';
import { interval, Subject } from 'rxjs';
import { takeUntil, switchMap } from 'rxjs/operators';
import { NbToastrService } from '@nebular/theme';
import { TranslateService } from '@ngx-translate/core';
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
    
    this.confirmDialog.confirmDelete(itemName, additionalWarning)
      .subscribe(confirmed => {
        if (!confirmed) {
          event.confirm.reject();
          return;
        }

        this.nodeService.deleteBackend(backend.IP, backend.HeartbeatPort)
          .subscribe({
            next: () => {
              this.toastr.success(
                this.translate.instant('nodes.backend.node_delete_success', { 
                  nodeType: nodeType, 
                  itemName: itemName 
                }),
                this.translate.instant('common.success')
              );
              event.confirm.resolve();
              this.loadBackends();
            },
            error: (error) => {
              this.toastr.danger(
                ErrorHandler.extractErrorMessage(error),
                this.translate.instant('common.error'),
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
    private toastr: NbToastrService,
    private confirmDialog: ConfirmDialogService,
    private translate: TranslateService
  ) {
    // Update table settings when language changes
    this.translate.onLangChange.pipe(takeUntil(this.destroy$)).subscribe(() => {
      this.updateTableSettings();
    });
    // Get clusterId from ClusterContextService
    this.clusterId = this.clusterContext.getActiveClusterId() || 0;
  }

  ngOnInit(): void {
    // Initialize table settings with translations
    this.updateTableSettings();
    
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
            this.pageTitle = this.translate.instant('nodes.backend.title_cn');
            this.deploymentModeText = this.translate.instant('cluster.form.shared_data');
            this.deploymentModeBadgeClass = 'badge-info';
          } else {
            this.pageTitle = this.translate.instant('nodes.backend.title_be');
            this.deploymentModeText = this.translate.instant('cluster.form.shared_nothing');
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
          this.toastr.danger(
            ErrorHandler.handleClusterError(error),
            this.translate.instant('common.error'),
          );
          this.source.load([]);
          this.loading = false;
        },
      });
  }

  private updateTableSettings(): void {
    this.settings = {
      ...this.settings,
      noDataMessage: this.translate.instant('nodes.backend.no_data'),
      actions: {
        ...this.settings.actions,
        columnTitle: this.translate.instant('common.action'),
      },
      columns: {
        BackendId: {
          title: 'BE ID',
          type: 'string',
          width: '8%',
        },
        IP: {
          title: this.translate.instant('nodes.backend.ip'),
          type: 'string',
        },
        HeartbeatPort: {
          title: this.translate.instant('nodes.backend.heartbeat_port'),
          type: 'string',
          width: '10%',
        },
        BePort: {
          title: this.translate.instant('nodes.backend.be_port'),
          type: 'string',
          width: '10%',
        },
        Alive: {
          title: this.translate.instant('nodes.backend.status'),
          type: 'html',
          width: '8%',
          valuePrepareFunction: (value: string) => {
            const status = value === 'true' ? 'success' : 'danger';
            const text = value === 'true' ? this.translate.instant('nodes.backend.online') : this.translate.instant('nodes.backend.offline');
            return `<span class="badge badge-${status}">${text}</span>`;
          },
        },
        TabletNum: {
          title: this.translate.instant('nodes.backend.tablet_num'),
          type: 'string',
          width: '10%',
        },
        DataUsedCapacity: {
          title: this.translate.instant('nodes.backend.data_used_capacity'),
          type: 'string',
        },
        TotalCapacity: {
          title: this.translate.instant('nodes.backend.total_capacity_full'),
          type: 'string',
        },
        UsedPct: {
          title: this.translate.instant('nodes.backend.disk_used_pct'),
          type: 'html',
          width: '10%',
          valuePrepareFunction: (value: string) => renderMetricBadge(value, this.diskThresholds),
        },
        CpuUsedPct: {
          title: this.translate.instant('nodes.backend.cpu_used_pct'),
          type: 'html',
          width: '12%',
          valuePrepareFunction: (value: string) => renderMetricBadge(value, this.cpuThresholds),
        },
        MemUsedPct: {
          title: this.translate.instant('nodes.backend.mem_used_pct'),
          type: 'html',
          width: '10%',
          valuePrepareFunction: (value: string) => renderMetricBadge(value, this.memoryThresholds),
        },
        NumRunningQueries: {
          title: this.translate.instant('nodes.backend.num_running_queries'),
          type: 'string',
          width: '10%',
        },
      },
    };
    this.source.refresh();
  }
}
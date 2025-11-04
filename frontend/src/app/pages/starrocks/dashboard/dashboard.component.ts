import { Component, OnInit, OnDestroy } from '@angular/core';
import { Router } from '@angular/router';
import { interval, Subject } from 'rxjs';
import { takeUntil, switchMap } from 'rxjs/operators';
import { NbToastrService } from '@nebular/theme';
import { ClusterService, Cluster, ClusterHealth } from '../../../@core/data/cluster.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';
import { PermissionService } from '../../../@core/data/permission.service';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';

interface ClusterCard {
  cluster: Cluster;
  health?: ClusterHealth;
  loading: boolean;
  isActive: boolean;
}

@Component({
  selector: 'ngx-dashboard',
  templateUrl: './dashboard.component.html',
  styleUrls: ['./dashboard.component.scss'],
})
export class DashboardComponent implements OnInit, OnDestroy {
  clusters: ClusterCard[] = [];
  loading = true;
  activeCluster: Cluster | null = null;
  hasClusterAccess = false;
  canListClusters = false;
  canCreateCluster = false;
  canUpdateCluster = false;
  canDeleteCluster = false;
  canActivateCluster = false;
  canViewActiveCluster = false;
  canViewClusterDetails = false;
  canViewBackends = false;
  canViewFrontends = false;
  canViewQueries = false;
  private permissionSignature = '';
  private destroy$ = new Subject<void>();

  constructor(
    private clusterService: ClusterService,
    private clusterContext: ClusterContextService,
    private toastrService: NbToastrService,
    private router: Router,
    private permissionService: PermissionService,
    private confirmDialogService: ConfirmDialogService,
  ) {}

  ngOnInit(): void {
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
        this.updateActiveStatus();
      });

    this.permissionService.permissions$
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => this.applyPermissionState());

    this.applyPermissionState();
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadClusters(): void {
    if (!this.canListClusters) {
      this.loading = false;
      return;
    }

    this.loading = true;
    this.clusterService.listClusters().subscribe({
      next: (clusters) => {
        // Update clusters, setting isActive based on backend response
        this.clusters = clusters.map((cluster) => ({
          cluster,
          loading: false,
          isActive: cluster.is_active,
        }));
        
        // Refresh active cluster from backend
        this.clusterContext.refreshActiveCluster();
        
        this.loadHealthStatus();
        this.loading = false;
      },
      error: (error) => {
        this.handleError(error);
        this.loading = false;
      },
    });
  }

  updateClusters(clusters: Cluster[]): void {
    // Update clusters, setting isActive based on backend is_active field
    this.clusters = clusters.map((cluster) => ({
      cluster,
      loading: false,
      isActive: cluster.is_active,
    }));
  }

  updateActiveStatus(): void {
    // isActive status now comes from backend
    // Just need to refresh the display
    this.clusters.forEach(card => {
      // Status is already set from loadClusters based on is_active field
    });
  }

  toggleActiveCluster(clusterCard: ClusterCard) {
    if (!this.canActivateCluster) {
      this.toastrService.warning('您没有激活集群的权限', '提示');
      return;
    }

    if (clusterCard.isActive) {
      this.toastrService.warning('此集群已是活跃状态', '提示');
      return;
    }
    this.clusterContext.setActiveCluster(clusterCard.cluster);
    this.toastrService.success(`已激活集群: ${clusterCard.cluster.name}`, '成功');
      
      // Reload clusters to update is_active status
      setTimeout(() => this.loadClusters(), 500);
  }

  loadHealthStatus(): void {
    if (!this.hasClusterAccess) {
      return;
    }

    this.clusters.forEach((clusterCard) => {
      clusterCard.loading = true;
      this.clusterService.getHealth(clusterCard.cluster.id).subscribe({
        next: (health) => {
          clusterCard.health = health;
          clusterCard.loading = false;
        },
        error: () => {
          clusterCard.loading = false;
        },
      });
    });
  }

  getStatusColor(status?: string): string {
    switch (status) {
      case 'healthy':
        return 'success';  // 绿色 - 健康
      case 'warning':
        return 'warning';  // 黄色 - 警告
      case 'critical':
        return 'danger';   // 红色 - 危险/不健康
      default:
        return 'basic';    // 默认 - 未知状态
    }
  }

  navigateToCluster(clusterId?: number): void {
    if (!this.canViewClusterDetails) {
      this.toastrService.warning('您没有查看集群详情的权限', '提示');
      return;
    }
    const commands = clusterId ? ['/pages/starrocks/clusters', clusterId] : ['/pages/starrocks/clusters'];
    this.router.navigate(commands);
  }

  navigateToBackends(clusterId?: number): void {
    if (!this.canViewBackends) {
      this.toastrService.warning('您没有查看 Backend 节点的权限', '提示');
      return;
    }
    const commands = clusterId ? ['/pages/starrocks/backends', clusterId] : ['/pages/starrocks/backends'];
    this.router.navigate(commands);
  }

  navigateToFrontends(clusterId?: number): void {
    if (!this.canViewFrontends) {
      this.toastrService.warning('您没有查看 Frontend 节点的权限', '提示');
      return;
    }
    const commands = clusterId ? ['/pages/starrocks/frontends', clusterId] : ['/pages/starrocks/frontends'];
    this.router.navigate(commands);
  }

  navigateToQueries(clusterId?: number): void {
    if (!this.canViewQueries) {
      this.toastrService.warning('您没有查看查询信息的权限', '提示');
      return;
    }
    const commands = clusterId ? ['/pages/starrocks/queries/execution', clusterId] : ['/pages/starrocks/queries/execution'];
    this.router.navigate(commands);
  }

  addCluster(): void {
    if (!this.canCreateCluster) {
      this.toastrService.warning('您没有创建集群的权限', '提示');
      return;
    }
    this.router.navigate(['/pages/starrocks/clusters/new']);
  }

  editCluster(cluster: Cluster): void {
    if (!this.canUpdateCluster) {
      this.toastrService.warning('您没有编辑集群的权限', '提示');
      return;
    }
    this.router.navigate(['/pages/starrocks/clusters', cluster.id, 'edit']);
  }

  deleteCluster(cluster: Cluster): void {
    if (!this.canDeleteCluster) {
      this.toastrService.warning('您没有删除集群的权限', '提示');
      return;
    }
    this.confirmDialogService.confirmDelete(cluster.name)
      .subscribe(confirmed => {
        if (!confirmed) {
          return;
        }

        this.clusterService.deleteCluster(cluster.id).subscribe({
          next: () => {
            this.toastrService.success(`集群 "${cluster.name}" 已删除`, '成功');
            this.loadClusters();
          },
          error: (error) => {
            this.handleError(error);
          },
        });
      });
  }

  private handleError(error: any): void {
    console.error('Error:', error);
    this.toastrService.danger(
      ErrorHandler.extractErrorMessage(error),
      '错误',
    );
  }

  private applyPermissionState(): void {
    const canList = this.permissionService.hasPermission('api:clusters:list');
    const canCreate = this.permissionService.hasPermission('api:clusters:create');
    const canUpdate = this.permissionService.hasPermission('api:clusters:update');
    const canDelete = this.permissionService.hasPermission('api:clusters:delete');
    const canActivate = this.permissionService.hasPermission('api:clusters:activate');
    const canViewActive = this.permissionService.hasPermission('api:clusters:active');
    const canViewDetail = this.permissionService.hasPermission('api:clusters:get');
    const canViewBackends = this.permissionService.hasPermission('api:clusters:backends');
    const canViewFrontends = this.permissionService.hasPermission('api:clusters:frontends');
    const canViewQuery = this.permissionService.hasPermission('api:clusters:queries');

    const signature = [
      canList,
      canCreate,
      canUpdate,
      canDelete,
      canActivate,
      canViewActive,
      canViewDetail,
      canViewBackends,
      canViewFrontends,
      canViewQuery,
    ]
      .map(flag => (flag ? '1' : '0'))
      .join('');

    const signatureChanged = signature !== this.permissionSignature;
    this.permissionSignature = signature;

    this.canListClusters = canList;
    this.canCreateCluster = canCreate;
    this.canUpdateCluster = canUpdate;
    this.canDeleteCluster = canDelete;
    this.canActivateCluster = canActivate;
    this.canViewActiveCluster = canViewActive;
    this.canViewClusterDetails = canViewDetail;
    this.canViewBackends = canViewBackends;
    this.canViewFrontends = canViewFrontends;
    this.canViewQueries = canViewQuery;
    this.hasClusterAccess = this.canListClusters;

    if (!this.hasClusterAccess) {
      this.loading = false;
      this.clusters = [];
      return;
    }

    if (signatureChanged && this.canListClusters) {
      this.loadClusters();
    }
  }
}


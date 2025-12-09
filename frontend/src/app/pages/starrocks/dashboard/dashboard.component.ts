import { Component, OnInit, OnDestroy, ChangeDetectionStrategy, ChangeDetectorRef } from '@angular/core';
import { Router } from '@angular/router';
import { interval, Subject } from 'rxjs';
import { takeUntil, switchMap } from 'rxjs/operators';
import { NbToastrService } from '@nebular/theme';
import { ClusterService, Cluster, ClusterHealth } from '../../../@core/data/cluster.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { OrganizationService, Organization } from '../../../@core/data/organization.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';
import { PermissionService } from '../../../@core/data/permission.service';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';
import { AuthService } from '../../../@core/data/auth.service';

interface ClusterCard {
  cluster: Cluster;
  health?: ClusterHealth;
  loading: boolean;
  isActive: boolean;
  organization?: Organization;
  showHealthDetails?: boolean;
}

@Component({
  selector: 'ngx-dashboard',
  templateUrl: './dashboard.component.html',
  styleUrls: ['./dashboard.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class DashboardComponent implements OnInit, OnDestroy {
  clusters: ClusterCard[] = [];
  loading = true;
  activeCluster: Cluster | null = null;
  hasClusterAccess = false;
  organizationsMap = new Map<number, Organization>();
  isSuperAdmin = false;
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
    private organizationService: OrganizationService,
    private toastrService: NbToastrService,
    private router: Router,
    private permissionService: PermissionService,
    private confirmDialogService: ConfirmDialogService,
    private authService: AuthService,
    private cdr: ChangeDetectorRef,
  ) {}

  ngOnInit(): void {
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
        this.updateActiveStatus();
        this.cdr.markForCheck();
      });

    this.permissionService.permissions$
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => {
        this.applyPermissionState();
        this.cdr.markForCheck();
      });

    this.applyPermissionState();
    
    // Load organizations if super admin
    this.isSuperAdmin = this.authService.isSuperAdmin();
    if (this.isSuperAdmin) {
      this.loadOrganizations();
    }
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  private loadOrganizations(): void {
    this.organizationService.listOrganizations().subscribe({
      next: (orgs) => {
        orgs.forEach(org => this.organizationsMap.set(org.id, org));
        this.cdr.markForCheck();
      },
      error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
    });
  }

  loadClusters(): void {
    if (!this.canListClusters) {
      this.loading = false;
      this.cdr.markForCheck();
      return;
    }

    this.loading = true;
    this.cdr.markForCheck();
    this.clusterService.listClusters().subscribe({
      next: (clusters) => {
        // Update clusters, setting isActive based on backend response
        this.clusters = clusters.map((cluster) => ({
          cluster,
          loading: false,
          isActive: cluster.is_active,
          organization: cluster.organization_id ? this.organizationsMap.get(cluster.organization_id) : undefined,
        }));
        
        // Refresh active cluster from backend
        this.clusterContext.refreshActiveCluster();
        
        this.loadHealthStatus();
        this.loading = false;
        this.cdr.markForCheck();
      },
      error: (error) => {
        this.handleError(error);
        this.loading = false;
        this.cdr.markForCheck();
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
    this.cdr.markForCheck();
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
    this.cdr.markForCheck();
      
      // Reload clusters to update is_active status
      setTimeout(() => this.loadClusters(), 500);
  }

  loadHealthStatus(): void {
    if (!this.hasClusterAccess) {
      return;
    }

    this.clusters.forEach((clusterCard) => {
      clusterCard.loading = true;
      this.cdr.markForCheck();
      this.clusterService.getHealth(clusterCard.cluster.id).subscribe({
        next: (health) => {
          clusterCard.health = health;
          clusterCard.loading = false;
          this.cdr.markForCheck();
        },
        error: () => {
          clusterCard.loading = false;
          this.cdr.markForCheck();
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

  // Get health badge status for nb-badge component
  getHealthBadgeStatus(clusterCard: ClusterCard): string {
    if (!clusterCard.health) {
      return 'basic';
    }
    const status = clusterCard.health.status;
    if (status === 'healthy') {
      return 'success';
    }
    if (status === 'warning') {
      return 'warning';
    }
    return 'danger';
  }

  // Get health badge text for nb-badge component
  getHealthBadgeText(clusterCard: ClusterCard): string {
    if (!clusterCard.health) {
      return '未知';
    }
    const status = clusterCard.health.status;
    if (status === 'healthy') {
      return '运行中';
    }
    if (status === 'warning') {
      return '警告';
    }
    return '异常';
  }

  // Get health badge text with last check time
  getHealthBadgeTextWithTime(clusterCard: ClusterCard): string {
    const statusText = this.getHealthBadgeText(clusterCard);
    if (!clusterCard.health?.last_check_time) {
      return statusText;
    }
    
    try {
      const checkTime = new Date(clusterCard.health.last_check_time);
      const now = new Date();
      const diffMs = now.getTime() - checkTime.getTime();
      const diffMins = Math.floor(diffMs / 60000);
      
      if (diffMins < 1) {
        return `${statusText} · 刚刚`;
      } else if (diffMins < 60) {
        return `${statusText} · ${diffMins}分钟前`;
      } else {
        const diffHours = Math.floor(diffMins / 60);
        return `${statusText} · ${diffHours}小时前`;
      }
    } catch (e) {
      return statusText;
    }
  }

  // Toggle health check details visibility
  toggleHealthDetails(clusterCard: ClusterCard): void {
    clusterCard.showHealthDetails = !clusterCard.showHealthDetails;
    this.cdr.markForCheck();
  }

  // Get health icon based on cluster health status
  getHealthIcon(clusterCard: ClusterCard): string {
    if (!clusterCard.health) {
      return 'question-mark-circle-outline';
    }
    const status = clusterCard.health.status;
    if (status === 'healthy') {
      return 'checkmark-circle-2-outline';
    }
    if (status === 'warning') {
      return 'alert-triangle-outline';
    }
    return 'close-circle-outline';
  }

  // Get failed health checks count
  getFailedChecksCount(clusterCard: ClusterCard): number {
    if (!clusterCard.health?.checks) {
      return 0;
    }
    return clusterCard.health.checks.filter(c => c.status !== 'ok').length;
  }

  // Get FE node count from health checks
  getFeCount(clusterCard: ClusterCard): string {
    if (!clusterCard.health?.checks) {
      return '—';
    }
    
    // Find Frontend Nodes check (backend now returns real FE count)
    const feCheck = clusterCard.health.checks.find(c => 
      c.name.toLowerCase().includes('frontend') || 
      c.name.toLowerCase().includes('fe')
    );
    
    if (feCheck && feCheck.message) {
      // Extract number from messages like:
      // "All 3 FE nodes are online"
      // "2/3 FE nodes are online"
      const match = feCheck.message.match(/(\d+)/);
      if (match) {
        return match[1];
      }
    }
    
    return '—';
  }

  // Get compute node count from health checks (BE for shared-nothing, CN for shared-data)
  getComputeNodeCount(clusterCard: ClusterCard): string {
    if (!clusterCard.health?.checks) {
      return '—';
    }
    // Look for "Compute Nodes" check (backend returns this name)
    const computeCheck = clusterCard.health.checks.find(c => 
      c.name.toLowerCase().includes('compute')
    );
    if (computeCheck?.message) {
      // Extract first number from message like "All 3 BE nodes are online"
      const match = computeCheck.message.match(/(\d+)/);
      return match ? match[1] : '—';
    }
    return '—';
  }

  // Check if cluster is shared-data mode
  isSharedData(clusterCard: ClusterCard): boolean {
    return clusterCard.cluster.deployment_mode === 'shared_data';
  }

  // Get compute node label based on deployment mode
  getComputeNodeLabel(clusterCard: ClusterCard): string {
    return this.isSharedData(clusterCard) ? 'CN 节点' : 'BE 节点';
  }

  // Calculate health score based on checks (return string for better display)
  getHealthScore(clusterCard: ClusterCard): string {
    if (!clusterCard.health?.checks || clusterCard.health.checks.length === 0) {
      return '—';
    }
    const totalChecks = clusterCard.health.checks.length;
    const passedChecks = clusterCard.health.checks.filter(c => c.status === 'ok').length;
    const score = Math.round((passedChecks / totalChecks) * 100);
    return `${score}`;
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
      this.toastrService.warning('您没有查看计算节点的权限', '提示');
      return;
    }
    
    // Activate cluster first if clicking from a specific cluster card
    if (clusterId) {
      const clusterCard = this.clusters.find(c => c.cluster.id === clusterId);
      if (clusterCard && !clusterCard.isActive) {
        this.clusterService.activateCluster(clusterId).subscribe({
          next: () => {
            this.router.navigate(['/pages/starrocks/backends']);
          },
          error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
        });
        return;
      }
    }
    
    // Navigate to backends page
    this.router.navigate(['/pages/starrocks/backends']);
  }

  navigateToFrontends(clusterId?: number): void {
    if (!this.canViewFrontends) {
      this.toastrService.warning('您没有查看 Frontend 节点的权限', '提示');
      return;
    }
    
    // Activate cluster first if clicking from a specific cluster card
    if (clusterId) {
      const clusterCard = this.clusters.find(c => c.cluster.id === clusterId);
      if (clusterCard && !clusterCard.isActive) {
        this.clusterService.activateCluster(clusterId).subscribe({
          next: () => {
            this.router.navigate(['/pages/starrocks/frontends']);
          },
          error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
        });
        return;
      }
    }
    
    // Navigate to frontends page
    this.router.navigate(['/pages/starrocks/frontends']);
  }

  navigateToQueries(clusterId?: number): void {
    if (!this.canViewQueries) {
      this.toastrService.warning('您没有查看查询信息的权限', '提示');
      return;
    }
    // 查询执行页不接收 ID，组件内部会使用 ActiveCluster 或自己解析
    this.router.navigate(['/pages/starrocks/queries/execution']);
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
            this.cdr.markForCheck();
          },
          error: (error) => {
            this.handleError(error);
            this.cdr.markForCheck();
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
      this.cdr.markForCheck();
      return;
    }

    if (signatureChanged && this.canListClusters) {
      this.loadClusters();
    }
    this.cdr.markForCheck();
  }
}


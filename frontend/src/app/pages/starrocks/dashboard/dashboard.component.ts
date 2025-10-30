import { Component, OnInit, OnDestroy } from '@angular/core';
import { Router } from '@angular/router';
import { interval, Subject } from 'rxjs';
import { takeUntil, switchMap } from 'rxjs/operators';
import { NbToastrService } from '@nebular/theme';
import { ClusterService, Cluster, ClusterHealth } from '../../../@core/data/cluster.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';

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
  private destroy$ = new Subject<void>();

  constructor(
    private clusterService: ClusterService,
    private clusterContext: ClusterContextService,
    private toastrService: NbToastrService,
    private router: Router,
  ) {}

  ngOnInit(): void {
    // Subscribe to active cluster changes
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
        this.updateActiveStatus();
      });

    this.loadClusters();
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadClusters(): void {
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

  navigateToCluster(): void {
    this.router.navigate(['/pages/starrocks/clusters']);
  }

  navigateToBackends(): void {
    this.router.navigate(['/pages/starrocks/backends']);
  }

  navigateToFrontends(): void {
    this.router.navigate(['/pages/starrocks/frontends']);
  }

  navigateToQueries(): void {
    this.router.navigate(['/pages/starrocks/queries/execution']);
  }

  addCluster(): void {
    this.router.navigate(['/pages/starrocks/clusters/new']);
  }

  editCluster(cluster: Cluster): void {
    this.router.navigate(['/pages/starrocks/clusters', cluster.id, 'edit']);
  }

  deleteCluster(cluster: Cluster): void {
    if (confirm(`确定要删除集群 "${cluster.name}" 吗？`)) {
      this.clusterService.deleteCluster(cluster.id).subscribe({
        next: () => {
          this.toastrService.success(`集群 "${cluster.name}" 已删除`, '成功');
          this.loadClusters(); // 重新加载集群列表
        },
        error: (error) => {
          this.handleError(error);
        }
      });
    }
  }

  private handleError(error: any): void {
    console.error('Error:', error);
    this.toastrService.danger(
      ErrorHandler.extractErrorMessage(error),
      '错误',
    );
  }
}


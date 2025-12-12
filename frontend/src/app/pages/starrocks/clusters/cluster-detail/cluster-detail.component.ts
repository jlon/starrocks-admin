import { Component, OnInit, TemplateRef } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { NbDialogService, NbToastrService } from '@nebular/theme';
import { ClusterService, Cluster, ClusterHealth } from '../../../../@core/data/cluster.service';
import { ConfirmDialogService } from '../../../../@core/services/confirm-dialog.service';
import { NodeService, Variable } from '../../../../@core/data/node.service';

@Component({
  selector: 'ngx-cluster-detail',
  templateUrl: './cluster-detail.component.html',
  styleUrls: ['./cluster-detail.component.scss'],
})
export class ClusterDetailComponent implements OnInit {
  cluster: Cluster | null = null;
  health: ClusterHealth | null = null;
  loading = true;
  clusterId: number;
  configLoading = false;
  configureInfo: Variable[] = [];
  filteredConfigureInfo: Variable[] = [];
  configSearchText = '';

  constructor(
    private clusterService: ClusterService,
    private route: ActivatedRoute,
    private router: Router,
    private toastrService: NbToastrService,
    private confirmDialogService: ConfirmDialogService,
    private nodeService: NodeService,
    private dialogService: NbDialogService,
  ) {
    this.clusterId = parseInt(this.route.snapshot.paramMap.get('id') || '0', 10);
  }

  ngOnInit(): void {
    this.loadCluster();
    this.loadHealth();
  }

  loadCluster(): void {
    this.loading = true;
    this.clusterService.getCluster(this.clusterId).subscribe({
      next: (cluster) => {
        this.cluster = cluster;
        this.loading = false;
      },
      error: (error) => {
        this.toastrService.danger(error.error?.message || '加载集群失败', '错误');
        this.loading = false;
      },
    });
  }

  loadHealth(): void {
    this.clusterService.getHealth(this.clusterId).subscribe({
      next: (health) => { this.health = health; },
      error: () => {},
    });
  }

  navigateTo(path: string): void {
    // Paths that require activating cluster first and then navigating to a global route
    const globalRoutes = ['queries', 'monitor', 'frontends', 'backends'];
    
    if (globalRoutes.includes(path) || path === 'queries') { // keep queries explicit for safety
      this.clusterService.activateCluster(this.clusterId).subscribe(() => {
        let routePath = '';
        switch (path) {
          case 'queries':
            routePath = '/pages/starrocks/queries/execution';
            break;
          case 'monitor':
            routePath = '/pages/starrocks/overview';
            break;
          case 'frontends':
            routePath = '/pages/starrocks/frontends';
            break;
          case 'backends':
            routePath = '/pages/starrocks/backends';
            break;
        }
        if (routePath) {
          this.router.navigate([routePath]);
        }
      });
    } else {
      // Fallback for any future routes that might actually take an ID
      this.router.navigate(['/pages/starrocks', path, this.clusterId]);
    }
  }

  editCluster(): void {
    this.router.navigate(['/pages/starrocks/clusters', this.clusterId, 'edit']);
  }

  deleteCluster(): void {
    const clusterName = this.cluster?.name || '';

    this.confirmDialogService.confirmDelete(clusterName)
      .subscribe(confirmed => {
        if (!confirmed) {
          return;
        }

        this.clusterService.deleteCluster(this.clusterId).subscribe({
          next: () => {
            this.toastrService.success('集群删除成功', '成功');
            this.router.navigate(['/pages/starrocks/clusters']);
          },
          error: (error) => {
            this.toastrService.danger(error.error?.message || '删除失败', '错误');
          },
        });
      });
  }

  openConfigureDialog(dialog: TemplateRef<any>): void {
    this.configLoading = true;
    this.configSearchText = '';
    this.dialogService.open(dialog, { closeOnBackdropClick: true });
    this.nodeService.getConfigureInfo().subscribe({
      next: (data) => {
        this.configureInfo = data;
        this.filteredConfigureInfo = data;
        this.configLoading = false;
      },
      error: (error) => {
        this.toastrService.danger(error.error?.message || '获取配置失败', '错误');
        this.configLoading = false;
      },
    });
  }

  filterConfig(): void {
    const search = this.configSearchText.toLowerCase().trim();
    this.filteredConfigureInfo = search
      ? this.configureInfo.filter(c => c.name.toLowerCase().includes(search) || (c.value && c.value.toLowerCase().includes(search)))
      : this.configureInfo;
  }
}

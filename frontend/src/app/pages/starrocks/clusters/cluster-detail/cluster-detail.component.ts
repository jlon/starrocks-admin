import { Component, OnInit } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';
import { NbToastrService } from '@nebular/theme';
import { ClusterService, Cluster, ClusterHealth } from '../../../../@core/data/cluster.service';
import { ConfirmDialogService } from '../../../../@core/services/confirm-dialog.service';

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

  constructor(
    private clusterService: ClusterService,
    private route: ActivatedRoute,
    private router: Router,
    private toastrService: NbToastrService,
    private confirmDialogService: ConfirmDialogService,
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
    if (path === 'queries') {
      this.router.navigate(['/pages/starrocks/queries/execution']);
    } else {
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
}

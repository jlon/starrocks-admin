import { Component, OnInit, OnDestroy } from '@angular/core';
import { Router } from '@angular/router';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { NbToastrService } from '@nebular/theme';
import { TranslateService } from '@ngx-translate/core';
import { ClusterService, Cluster } from '../../../@core/data/cluster.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';

@Component({
  selector: 'ngx-cluster-selector',
  templateUrl: './cluster-selector.component.html',
  styleUrls: ['./cluster-selector.component.scss'],
})
export class ClusterSelectorComponent implements OnInit, OnDestroy {
  clusters: Cluster[] = [];
  activeCluster: Cluster | null = null;
  loading = false;
  private destroy$ = new Subject<void>();

  constructor(
    private clusterService: ClusterService,
    private clusterContext: ClusterContextService,
    private router: Router,
    private toastr: NbToastrService,
    private translate: TranslateService,
  ) {}

  ngOnInit(): void {
    // Subscribe to active cluster changes
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
      });

    // Load clusters
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
        this.clusters = clusters;
        this.loading = false;

        // The active cluster status comes from backend via the is_active field
        // Just need to refresh if no active cluster is shown
        if (clusters.length > 0 && !this.activeCluster) {
          // Refresh active cluster from backend
          this.clusterContext.refreshActiveCluster();
        }
      },
      error: (error) => {
        this.toastr.danger(this.translate.instant('cluster.load_list_failed'), this.translate.instant('cluster.error'));
        this.loading = false;
      },
    });
  }

  selectCluster(cluster: Cluster): void {
    this.clusterContext.setActiveCluster(cluster);
    this.toastr.success(
      this.translate.instant('cluster.switch_success', { name: cluster.name }),
      this.translate.instant('cluster.success')
    );
  }

  onClusterChange(cluster: Cluster): void {
    if (cluster) {
      this.selectCluster(cluster);
    }
  }

  compareById(c1: Cluster, c2: Cluster): boolean {
    return c1 && c2 ? c1.id === c2.id : c1 === c2;
  }

  goToClusterManagement(): void {
    this.router.navigate(['/pages/starrocks/clusters']);
  }

  refreshClusters(): void {
    this.loadClusters();
  }
}


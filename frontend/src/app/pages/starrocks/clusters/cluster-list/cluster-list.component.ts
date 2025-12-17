import { Component, OnInit, OnDestroy } from '@angular/core';
import { Router } from '@angular/router';
import { NbDialogService, NbToastrService } from '@nebular/theme';
import { TranslateService } from '@ngx-translate/core';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { ClusterService, Cluster } from '../../../../@core/data/cluster.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { ConfirmDialogService } from '../../../../@core/services/confirm-dialog.service';

@Component({
  selector: 'ngx-cluster-list',
  templateUrl: './cluster-list.component.html',
  styleUrls: ['./cluster-list.component.scss'],
})
export class ClusterListComponent implements OnInit, OnDestroy {
  source: LocalDataSource = new LocalDataSource();
  loading = true;
  private destroy$ = new Subject<void>();

  settings = {
    mode: 'external',
    hideSubHeader: false,  // Enable search
    noDataMessage: '暂无集群数据，点击上方按钮添加集群',
    actions: {
      columnTitle: '操作',
      add: false,
      edit: true,
      delete: true,
      position: 'right',
    },
    edit: {
      editButtonContent: '<i class="nb-edit"></i>',
    },
    delete: {
      deleteButtonContent: '<i class="nb-trash"></i>',
      confirmDelete: false,
    },
    pager: {
      display: true,
      perPage: 10,
    },
    columns: {
      id: {
        title: 'ID',
        type: 'number',
        width: '5%',
      },
      name: {
        title: '集群名称',
        type: 'string',
      },
      fe_host: {
        title: 'FE 地址',
        type: 'string',
      },
      fe_http_port: {
        title: 'HTTP 端口',
        type: 'number',
        width: '10%',
      },
      fe_query_port: {
        title: '查询端口',
        type: 'number',
        width: '10%',
      },
      username: {
        title: '用户名',
        type: 'string',
        width: '10%',
      },
      description: {
        title: '描述',
        type: 'string',
      },
      created_at: {
        title: '创建时间',
        type: 'string',
        valuePrepareFunction: (date: string) => {
          return new Date(date).toLocaleString('zh-CN');
        },
      },
    },
  };

  constructor(
    private clusterService: ClusterService,
    private router: Router,
    private dialogService: NbDialogService,
    private toastrService: NbToastrService,
    private confirmDialogService: ConfirmDialogService,
    private translate: TranslateService,
  ) {
    // Update table settings when language changes
    this.translate.onLangChange.pipe(takeUntil(this.destroy$)).subscribe(() => {
      this.updateTableSettings();
    });
  }

  ngOnInit(): void {
    // Initialize table settings with translations
    this.updateTableSettings();
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
        this.source.load(clusters);
        this.loading = false;
      },
      error: (error) => {
        this.toastrService.danger(
          ErrorHandler.extractErrorMessage(error),
          this.translate.instant('common.error'),
        );
        this.loading = false;
      },
    });
  }

  onCreate(): void {
    this.router.navigate(['/pages/starrocks/clusters/new']);
  }

  onEdit(event: any): void {
    this.router.navigate(['/pages/starrocks/clusters', event.data.id, 'edit']);
  }

  onDelete(event: any): void {
    const cluster = event.data as Cluster;

    this.confirmDialogService.confirmDelete(cluster.name)
      .subscribe(confirmed => {
        if (!confirmed) {
          return;
        }

        this.clusterService.deleteCluster(cluster.id).subscribe({
          next: () => {
            this.toastrService.success(
              this.translate.instant('cluster.delete_success_simple'),
              this.translate.instant('common.success')
            );
            this.loadClusters();
          },
          error: (error) => {
            this.toastrService.danger(
              ErrorHandler.extractErrorMessage(error),
              this.translate.instant('common.error'),
            );
          },
        });
      });
  }

  onRowSelect(event: any): void {
    this.router.navigate(['/pages/starrocks/clusters', event.data.id]);
  }

  testConnection(cluster: Cluster): void {
    this.clusterService.getHealth(cluster.id).subscribe({
      next: (health) => {
        if (health.status === 'healthy') {
          const details = health.checks.map(c => `${c.name}: ${c.message}`).join('\n');
          this.toastrService.success(
            details,
            this.translate.instant('cluster.health_check_passed')
          );
        } else if (health.status === 'warning') {
          const details = health.checks.map(c => `${c.name}: ${c.message}`).join('\n');
          this.toastrService.warning(details, this.translate.instant('cluster.health_check_warning'));
        } else {
          const details = health.checks.map(c => `${c.name}: ${c.message}`).join('\n');
          this.toastrService.danger(details, this.translate.instant('cluster.health_check_failed'));
        }
      },
      error: (error) => {
        this.toastrService.danger(
          ErrorHandler.extractErrorMessage(error),
          this.translate.instant('common.error'),
        );
      },
    });
  }

  private updateTableSettings(): void {
    this.settings = {
      ...this.settings,
      noDataMessage: this.translate.instant('cluster.no_clusters_hint'),
      actions: {
        columnTitle: this.translate.instant('common.action'),
        add: false,
        edit: true,
        delete: true,
        position: 'right',
      },
      edit: {
        editButtonContent: '<i class="nb-edit"></i>',
      },
      delete: {
        deleteButtonContent: '<i class="nb-trash"></i>',
        confirmDelete: false,
      },
      pager: {
        display: true,
        perPage: 10,
      },
      columns: {
        id: {
          title: 'ID',
          type: 'number',
          width: '5%',
        },
        name: {
          title: this.translate.instant('cluster.form.name_label'),
          type: 'string',
        },
        fe_host: {
          title: this.translate.instant('cluster.form.fe_host_label'),
          type: 'string',
        },
        fe_http_port: {
          title: this.translate.instant('cluster.form.http_port_label'),
          type: 'number',
          width: '10%',
        },
        fe_query_port: {
          title: this.translate.instant('cluster.form.query_port_label'),
          type: 'number',
          width: '10%',
        },
        username: {
          title: this.translate.instant('cluster.form.username_label'),
          type: 'string',
          width: '10%',
        },
        description: {
          title: this.translate.instant('cluster.form.description_label'),
          type: 'string',
        },
        created_at: {
          title: this.translate.instant('cluster.created_at'),
          type: 'string',
          valuePrepareFunction: (date: string) => {
            return new Date(date).toLocaleString('zh-CN');
          },
        },
      },
    };
    this.source.refresh();
  }
}


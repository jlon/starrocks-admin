import { Component, OnInit } from '@angular/core';
import { Router } from '@angular/router';
import { NbDialogService, NbToastrService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { ClusterService, Cluster } from '../../../../@core/data/cluster.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { ConfirmDialogService } from '../../../../@core/services/confirm-dialog.service';

@Component({
  selector: 'ngx-cluster-list',
  templateUrl: './cluster-list.component.html',
  styleUrls: ['./cluster-list.component.scss'],
})
export class ClusterListComponent implements OnInit {
  source: LocalDataSource = new LocalDataSource();
  loading = true;

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
  ) {}

  ngOnInit(): void {
    this.loadClusters();
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
          '错误',
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
            this.toastrService.success('集群删除成功', '成功');
            this.loadClusters();
          },
          error: (error) => {
            this.toastrService.danger(
              ErrorHandler.extractErrorMessage(error),
              '错误',
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
          this.toastrService.success(details, '健康检查通过');
        } else if (health.status === 'warning') {
          const details = health.checks.map(c => `${c.name}: ${c.message}`).join('\n');
          this.toastrService.warning(details, '健康检查警告');
        } else {
          const details = health.checks.map(c => `${c.name}: ${c.message}`).join('\n');
          this.toastrService.danger(details, '健康检查失败');
        }
      },
      error: (error) => {
        this.toastrService.danger(
          ErrorHandler.extractErrorMessage(error),
          '错误',
        );
      },
    });
  }
}


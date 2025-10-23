import { Component, OnInit, OnDestroy } from '@angular/core';

import { NbToastrService, NbDialogService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { Cluster } from '../../../@core/data/cluster.service';
import { NodeService, Variable } from '../../../@core/data/node.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';

@Component({
  selector: 'ngx-variables',
  templateUrl: './variables.component.html',
  styleUrls: ['./variables.component.scss'],
})
export class VariablesComponent implements OnInit, OnDestroy {
  clusterId: number;
  activeCluster: Cluster | null = null;
  variables: Variable[] = [];
  source: LocalDataSource = new LocalDataSource();
  loading = false;
  searchText = '';
  variableType = 'global'; // 'global' or 'session'
  private destroy$ = new Subject<void>();

  settings = {
    hideSubHeader: false, // Enable search
    noDataMessage: '未找到匹配的变量',
    actions: {
      add: false,
      edit: true,
      delete: false,
      position: 'right',
    },
    edit: {
      editButtonContent: '<i class="nb-edit"></i>',
    },
    pager: {
      display: true,
      perPage: 20,
    },
    columns: {
      name: {
        title: 'Variable Name',
        type: 'string',
        width: '40%',
      },
      value: {
        title: 'Value',
        type: 'string',
        width: '60%',
        valuePrepareFunction: (value: any) => {
          if (!value) return 'NULL';
          return value.length > 200 ? value.substring(0, 200) + '...' : value;
        },
      },
    },
  };

  constructor(
    
    private toastrService: NbToastrService,
    private dialogService: NbDialogService,
    private clusterContext: ClusterContextService,
    private nodeService: NodeService,
  ) {
    // Try to get clusterId from route first
    // Get clusterId from ClusterContextService
    this.clusterId = this.clusterContext.getActiveClusterId() || 0;
  }

  ngOnInit(): void {
    console.log('[Variables] ngOnInit - Initial clusterId:', this.clusterId);
    
    // Subscribe to active cluster changes
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        console.log('[Variables] Active cluster changed:', cluster);
        this.activeCluster = cluster;
        if (cluster) {
          // Always use the active cluster (override route parameter)
          const newClusterId = cluster.id;
          if (this.clusterId !== newClusterId) {
            console.log('[Variables] Switching cluster from', this.clusterId, 'to', newClusterId);
            this.clusterId = newClusterId;
            this.loadVariables();
          }
        }
      });

    // Load variables if clusterId is already set
    if (this.clusterId && this.clusterId > 0) {
      console.log('[Variables] Loading with route clusterId:', this.clusterId);
      this.loadVariables();
    } else if (!this.clusterContext.hasActiveCluster()) {
      console.log('[Variables] No active cluster found');
      this.toastrService.warning('请先在集群概览页面激活一个集群', '提示');
    }
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadVariables(): void {
    if (!this.clusterId || this.clusterId === 0) {
      console.log('[Variables] No valid clusterId, skipping load');
      this.loading = false;
      return;
    }

    console.log('[Variables] Loading variables for cluster:', this.clusterId, 'type:', this.variableType);
    this.loading = true;
    this.nodeService.getVariables(
      this.clusterId,
      this.variableType,
      this.searchText || undefined
    ).subscribe({
      next: (variables) => {
        console.log('[Variables] Loaded variables:', variables.length);
        this.variables = variables;
        this.source.load(variables);
        this.loading = false;
      },
      error: (error) => {
        console.error('[Variables] Error loading variables:', error);
        this.toastrService.danger(
          error.error?.message || '加载变量失败',
          '错误'
        );
        this.variables = [];
        this.source.load([]);
        this.loading = false;
      },
    });
  }

  onEdit(event: any): void {
    this.editVariable(event.data);
  }

  editVariable(variable: Variable): void {
    const newValue = prompt(`修改变量 "${variable.name}":`, variable.value);
    if (newValue !== null && newValue !== variable.value) {
      this.loading = true;
      this.nodeService.updateVariable(this.clusterId, variable.name, {
        value: newValue,
        scope: this.variableType.toUpperCase(),
      }).subscribe({
        next: () => {
          this.toastrService.success(`变量 "${variable.name}" 更新成功`, '成功');
          this.loadVariables();
        },
        error: (error) => {
          this.toastrService.danger(
            error.error?.message || '更新变量失败',
            '错误'
          );
          this.loading = false;
        },
      });
    }
  }

  onTypeChange(): void {
    this.loadVariables();
  }

  onSearch(): void {
    this.loadVariables();
  }

  refresh(): void {
    this.loadVariables();
  }
}


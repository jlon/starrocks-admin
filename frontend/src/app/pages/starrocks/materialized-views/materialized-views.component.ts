import { Component, OnInit, OnDestroy, TemplateRef, ViewChild } from '@angular/core';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { NbToastrService, NbDialogService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import {
  MaterializedViewService,
  MaterializedView,
} from '../../../@core/data/materialized-view.service';
import { ClusterService, Cluster } from '../../../@core/data/cluster.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';
import { ActiveToggleRenderComponent } from './active-toggle-render.component';

@Component({
  selector: 'ngx-materialized-views',
  templateUrl: './materialized-views.component.html',
  styleUrls: ['./materialized-views.component.scss'],
  providers: [MaterializedViewService],
})
export class MaterializedViewsComponent implements OnInit, OnDestroy {
  @ViewChild('createDialog', { static: false }) createDialogTemplate: TemplateRef<any>;
  @ViewChild('detailDialog', { static: false }) detailDialogTemplate: TemplateRef<any>;
  @ViewChild('refreshDialog', { static: false }) refreshDialogTemplate: TemplateRef<any>;
  @ViewChild('editDialog', { static: false }) editDialogTemplate: TemplateRef<any>;

  source: LocalDataSource = new LocalDataSource();
  allMaterializedViews: MaterializedView[] = [];
  filteredMaterializedViews: MaterializedView[] = [];
  clusterId: number;
  activeCluster: Cluster | null = null;
  loading = true;
  private destroy$ = new Subject<void>();

  // Filter states
  searchText = '';
  selectedDatabase = 'all';
  selectedRefreshType = 'all';
  selectedActiveState = 'all';
  selectedRefreshState = 'all';
  showAdvancedFilters = false;

  // Advanced filters
  refreshTimeStart: string = '';
  refreshTimeEnd: string = '';
  rowCountMin: number | null = null;
  rowCountMax: number | null = null;
  selectedPartitionType = 'all';

  // Options for filters
  databases: string[] = [];
  refreshTypeOptions = [
    { value: 'all', label: '全部类型' },
    { value: 'ASYNC', label: '自动刷新' },
    { value: 'MANUAL', label: '手动刷新' },
    { value: 'ROLLUP', label: '同步' },
    { value: 'INCREMENTAL', label: '增量' },
  ];
  activeStateOptions = [
    { value: 'all', label: '全部' },
    { value: 'active', label: 'Active' },
    { value: 'inactive', label: 'Inactive' },
  ];
  refreshStateOptions = [
    { value: 'all', label: '全部' },
    { value: 'SUCCESS', label: '成功' },
    { value: 'RUNNING', label: '运行中' },
    { value: 'FAILED', label: '失败' },
    { value: 'PENDING', label: '等待中' },
  ];
  partitionTypeOptions = [
    { value: 'all', label: '全部分区类型' },
    { value: 'RANGE', label: 'RANGE' },
    { value: 'LIST', label: 'LIST' },
    { value: 'UNPARTITIONED', label: 'UNPARTITIONED' },
  ];

  // Statistics
  totalCount = 0;
  filteredCount = 0;
  activeCount = 0;
  inactiveCount = 0;

  // Dialog states
  createDialogRef: any;
  detailDialogRef: any;
  refreshDialogRef: any;
  editDialogRef: any;
  selectedMV: MaterializedView | null = null;
  mvDDL = '';

  // Create form
  createSQL = '';
  creating = false;

  // Refresh form
  refreshMode = 'ASYNC';
  refreshForce = false;
  refreshPartitionStart = '';
  refreshPartitionEnd = '';
  refreshing = false;

  // Edit form
  editAction = 'rename'; // rename | refresh_strategy | properties | advanced
  editNewName = '';
  editRefreshStrategy = 'MANUAL';
  editRefreshInterval = '1';
  editRefreshUnit = 'HOUR'; // HOUR | DAY | WEEK | MONTH
  editPropertyKey = '';
  editPropertyValue = '';
  editAdvancedClause = '';
  editing = false;

  refreshModeOptions = [
    { value: 'ASYNC', label: '异步模式' },
    { value: 'SYNC', label: '同步模式' },
  ];

  settings = {
    mode: 'external',
    hideSubHeader: false,
    noDataMessage: '暂无物化视图数据',
    actions: {
      columnTitle: '操作',
      add: false,
      edit: true,
      delete: true,
      position: 'right',
    },
    edit: {
      editButtonContent: '<i class="nb-search"></i>',
      confirmEdit: false,
    },
    delete: {
      deleteButtonContent: '<i class="nb-trash"></i>',
      confirmDelete: false,
    },
    pager: {
      display: true,
      perPage: 15,
    },
    columns: {
      name: {
        title: '名称',
        type: 'string',
        width: '12%',
      },
      database_name: {
        title: '数据库',
        type: 'string',
        width: '10%',
      },
      mv_type: {
        title: '类型',
        type: 'html',
        width: '7%',
        valuePrepareFunction: (value: any, row: MaterializedView) => {
          if (row.refresh_type === 'ROLLUP') {
            return '<span class="badge badge-primary">同步</span>';
          } else {
            return '<span class="badge badge-info">异步</span>';
          }
        },
      },
      refresh_type: {
        title: '刷新策略',
        type: 'html',
        width: '9%',
        valuePrepareFunction: (value: string) => {
          return this.getRefreshTypeBadge(value);
        },
      },
      is_active: {
        title: '状态',
        type: 'custom',
        width: '12%',
        renderComponent: ActiveToggleRenderComponent,
        onComponentInitFunction: (instance: any) => {
          instance.toggle.subscribe((rowData: any) => {
            this.toggleActiveState(rowData);
          });
        },
      },
      last_refresh_state: {
        title: '刷新状态',
        type: 'html',
        width: '9%',
        valuePrepareFunction: (value: string, row: MaterializedView) => {
          if (row.refresh_type === 'ROLLUP') return '-';
          return this.getRefreshStateBadge(value);
        },
      },
      last_refresh_finished_time: {
        title: '最后刷新时间',
        type: 'string',
        width: '15%',
        valuePrepareFunction: (value: string) => value || '-',
      },
      rows: {
        title: '行数',
        type: 'string',
        width: '8%',
        valuePrepareFunction: (value: number) => {
          if (value === null || value === undefined) return '-';
          return this.formatNumber(value);
        },
      },
      partition_type: {
        title: '分区类型',
        type: 'string',
        width: '8%',
        valuePrepareFunction: (value: string) => value || '-',
      },
      error_info: {
        title: '错误信息',
        type: 'html',
        width: '8%',
        valuePrepareFunction: (value: any, row: MaterializedView) => {
          if (row.last_refresh_error_message) {
            return `<span class="text-danger" title="${row.last_refresh_error_message}">
              <i class="nb-alert-circle"></i> 错误
            </span>`;
          }
          return '-';
        },
      },
    },
  };

  constructor(
    private mvService: MaterializedViewService,
    private clusterService: ClusterService,
    private clusterContextService: ClusterContextService,
    private toastrService: NbToastrService,
    private confirmDialogService: ConfirmDialogService,
    private dialogService: NbDialogService,
  ) {}

  ngOnInit() {
    // Get clusterId from ClusterContextService
    this.clusterId = this.clusterContextService.getActiveClusterId() || 0;

    // Subscribe to active cluster changes
    this.clusterContextService.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe((cluster) => {
        this.activeCluster = cluster;
        if (cluster) {
          const newClusterId = cluster.id;
          if (this.clusterId !== newClusterId) {
            this.clusterId = newClusterId;
            this.loadClusterInfo();
            this.loadMaterializedViews();
          }
        }
        // Backend will handle "no active cluster" case
      });

    // Load data - backend will get active cluster automatically
    this.loadClusterInfo();
    this.loadMaterializedViews();
  }

  ngOnDestroy() {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadClusterInfo() {
    this.clusterService
      .getCluster(this.clusterId)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (cluster) => {
          this.activeCluster = cluster;
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.extractErrorMessage(error),
            '加载集群信息失败',
          );
        },
      });
  }

  loadMaterializedViews() {
    this.loading = true;
    this.mvService
      .getMaterializedViews()
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (data) => {
          this.allMaterializedViews = data;
          this.extractDatabases();
          this.calculateStatistics();
          this.applyFilters();
          this.loading = false;
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.handleClusterError(error),
            '加载物化视图失败',
          );
          this.loading = false;
        },
      });
  }

  extractDatabases() {
    const dbSet = new Set<string>();
    this.allMaterializedViews.forEach((mv) => {
      if (mv && mv.database_name) {
        dbSet.add(mv.database_name);
      }
    });
    this.databases = Array.from(dbSet).sort();
  }

  calculateStatistics() {
    this.totalCount = this.allMaterializedViews.length;
    this.activeCount = this.allMaterializedViews.filter((mv) => mv && mv.is_active).length;
    this.inactiveCount = this.totalCount - this.activeCount;
  }

  applyFilters() {
    let filtered = [...this.allMaterializedViews];

    // Search filter
    if (this.searchText.trim()) {
      const searchLower = this.searchText.toLowerCase();
      filtered = filtered.filter(
        (mv) =>
          (mv.name && mv.name.toLowerCase().includes(searchLower)) ||
          (mv.database_name && mv.database_name.toLowerCase().includes(searchLower)),
      );
    }

    // Database filter
    if (this.selectedDatabase !== 'all') {
      filtered = filtered.filter((mv) => mv && mv.database_name === this.selectedDatabase);
    }

    // Refresh type filter
    if (this.selectedRefreshType !== 'all') {
      filtered = filtered.filter((mv) => mv && mv.refresh_type === this.selectedRefreshType);
    }

    // Active state filter
    if (this.selectedActiveState !== 'all') {
      const isActive = this.selectedActiveState === 'active';
      filtered = filtered.filter((mv) => mv && mv.is_active === isActive);
    }

    // Refresh state filter
    if (this.selectedRefreshState !== 'all') {
      filtered = filtered.filter(
        (mv) => mv && mv.last_refresh_state === this.selectedRefreshState,
      );
    }

    // Advanced filters
    if (this.showAdvancedFilters) {
      // Refresh time filter
      if (this.refreshTimeStart) {
        filtered = filtered.filter(
          (mv) =>
            mv && mv.last_refresh_finished_time &&
            mv.last_refresh_finished_time >= this.refreshTimeStart,
        );
      }
      if (this.refreshTimeEnd) {
        filtered = filtered.filter(
          (mv) =>
            mv && mv.last_refresh_finished_time &&
            mv.last_refresh_finished_time <= this.refreshTimeEnd,
        );
      }

      // Row count filter
      if (this.rowCountMin !== null) {
        filtered = filtered.filter((mv) => mv && mv.rows && mv.rows >= this.rowCountMin);
      }
      if (this.rowCountMax !== null) {
        filtered = filtered.filter((mv) => mv && mv.rows && mv.rows <= this.rowCountMax);
      }

      // Partition type filter
      if (this.selectedPartitionType !== 'all') {
        filtered = filtered.filter(
          (mv) => mv && mv.partition_type === this.selectedPartitionType,
        );
      }
    }

    this.filteredMaterializedViews = filtered;
    this.filteredCount = filtered.length;
    this.source.load(filtered);
  }

  onSearch() {
    this.applyFilters();
  }

  clearAllFilters() {
    this.searchText = '';
    this.selectedDatabase = 'all';
    this.selectedRefreshType = 'all';
    this.selectedActiveState = 'all';
    this.selectedRefreshState = 'all';
    this.refreshTimeStart = '';
    this.refreshTimeEnd = '';
    this.rowCountMin = null;
    this.rowCountMax = null;
    this.selectedPartitionType = 'all';
    this.applyFilters();
  }

  toggleAdvancedFilters() {
    this.showAdvancedFilters = !this.showAdvancedFilters;
  }

  getActiveFiltersCount(): number {
    let count = 0;
    if (this.searchText.trim()) count++;
    if (this.selectedDatabase !== 'all') count++;
    if (this.selectedRefreshType !== 'all') count++;
    if (this.selectedActiveState !== 'all') count++;
    if (this.selectedRefreshState !== 'all') count++;
    if (this.refreshTimeStart) count++;
    if (this.refreshTimeEnd) count++;
    if (this.rowCountMin !== null) count++;
    if (this.rowCountMax !== null) count++;
    if (this.selectedPartitionType !== 'all') count++;
    return count;
  }

  onEdit(event: any) {
    const mv = event.data as MaterializedView;
    this.viewDetail(mv);
  }

  onDelete(event: any) {
    const mv = event.data as MaterializedView;

    this.confirmDialogService.confirmDelete(mv.name)
      .subscribe(confirmed => {
        if (!confirmed) {
          return;
        }

        this.deleteMV(mv, event);
      });
  }

  // Check if refresh action should be shown
  canRefresh(mv: MaterializedView): boolean {
    return mv.refresh_type !== 'ROLLUP';
  }

  // Check if cancel action should be shown
  canCancelRefresh(mv: MaterializedView): boolean {
    return mv.refresh_type !== 'ROLLUP' && mv.last_refresh_state === 'RUNNING';
  }

  openCreateDialog() {
    this.createSQL = '';
    this.creating = false;
    this.createDialogRef = this.dialogService.open(this.createDialogTemplate, {
      context: {},
    });
  }

  closeCreateDialog() {
    if (this.createDialogRef) {
      this.createDialogRef.close();
    }
  }

  createMV() {
    if (!this.createSQL.trim()) {
      this.toastrService.warning('请输入CREATE MATERIALIZED VIEW SQL语句', '输入错误');
      return;
    }

    this.creating = true;
    this.mvService
      .createMaterializedView( { sql: this.createSQL })
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: () => {
          this.toastrService.success('物化视图创建成功', '成功');
          this.closeCreateDialog();
          this.loadMaterializedViews();
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.extractErrorMessage(error),
            '创建物化视图失败',
          );
          this.creating = false;
        },
      });
  }

  viewDetail(mv: MaterializedView) {
    this.selectedMV = mv;
    this.mvDDL = '';
    
    // Load DDL
    this.mvService
      .getMaterializedViewDDL( mv.name)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (result) => {
          this.mvDDL = result.ddl;
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.extractErrorMessage(error),
            '加载DDL失败',
          );
        },
      });

    this.detailDialogRef = this.dialogService.open(this.detailDialogTemplate, {
      context: {},
    });
  }

  closeDetailDialog() {
    if (this.detailDialogRef) {
      this.detailDialogRef.close();
    }
  }

  openRefreshDialog(mv: MaterializedView) {
    this.selectedMV = mv;
    this.refreshMode = 'ASYNC';
    this.refreshForce = false;
    this.refreshPartitionStart = '';
    this.refreshPartitionEnd = '';
    this.refreshing = false;

    this.refreshDialogRef = this.dialogService.open(this.refreshDialogTemplate, {
      context: {},
    });
  }

  closeRefreshDialog() {
    if (this.refreshDialogRef) {
      this.refreshDialogRef.close();
    }
  }

  refreshMV() {
    if (!this.selectedMV) return;

    this.refreshing = true;
    this.mvService
      .refreshMaterializedView( this.selectedMV.name, {
        mode: this.refreshMode,
        force: this.refreshForce,
        partition_start: this.refreshPartitionStart || undefined,
        partition_end: this.refreshPartitionEnd || undefined,
      })
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: () => {
          this.toastrService.success('刷新任务已启动', '成功');
          this.closeRefreshDialog();
          setTimeout(() => this.loadMaterializedViews(), 1000);
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.extractErrorMessage(error),
            '刷新失败',
          );
          this.refreshing = false;
        },
      });
  }

  cancelRefresh(mv: MaterializedView) {
    this.confirmDialogService
      .confirm(
        '取消刷新',
        `确定要取消物化视图 "${mv.name}" 的刷新任务吗？`,
        '取消刷新',
        '不取消',
      )
      .subscribe((confirmed) => {
        if (confirmed) {
          this.mvService
            .cancelRefreshMaterializedView( mv.name, false)
            .pipe(takeUntil(this.destroy$))
            .subscribe({
              next: () => {
                this.toastrService.success('刷新任务已取消', '成功');
                this.loadMaterializedViews();
              },
              error: (error) => {
                this.toastrService.danger(
                  ErrorHandler.extractErrorMessage(error),
                  '取消刷新失败',
                );
              },
            });
        }
      });
  }

  deleteMV(mv: MaterializedView, tableEvent?: any) {
    this.mvService
      .deleteMaterializedView( mv.name, true)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: () => {
          this.toastrService.success('物化视图删除成功', '成功');
          tableEvent?.confirm.resolve();
          this.loadMaterializedViews();
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.extractErrorMessage(error),
            '删除失败',
          );
          tableEvent?.confirm.reject();
        },
      });
  }

  // Toggle Active/Inactive state
  toggleActiveState(mv: MaterializedView) {
    const newState = mv.is_active ? 'INACTIVE' : 'ACTIVE';
    const action = mv.is_active ? '停用' : '激活';
    
    this.confirmDialogService
      .confirm(
        `${action}物化视图`,
        `确定要${action}物化视图 "${mv.name}" 吗？`,
        action,
        '取消',
      )
      .subscribe((confirmed) => {
        if (confirmed) {
          this.mvService
            .alterMaterializedView( mv.name, { alter_clause: newState })
            .pipe(takeUntil(this.destroy$))
            .subscribe({
              next: () => {
                this.toastrService.success(`物化视图已${action}`, '成功');
                this.loadMaterializedViews();
              },
              error: (error) => {
                this.toastrService.danger(
                  ErrorHandler.extractErrorMessage(error),
                  `${action}失败`,
                );
              },
            });
        }
      });
  }

  // Open edit dialog
  openEditDialog(mv: MaterializedView) {
    this.selectedMV = mv;
    this.editAction = 'rename';
    this.editNewName = mv.name;
    this.editRefreshStrategy = 'MANUAL';
    this.editRefreshInterval = '1';
    this.editRefreshUnit = 'HOUR';
    this.editPropertyKey = '';
    this.editPropertyValue = '';
    this.editAdvancedClause = '';
    this.editing = false;

    this.editDialogRef = this.dialogService.open(this.editDialogTemplate, {
      context: {},
    });
  }

  closeEditDialog() {
    if (this.editDialogRef) {
      this.editDialogRef.close();
    }
  }

  // Execute edit action
  editMV() {
    if (!this.selectedMV) return;

    let alterClause = '';
    
    switch (this.editAction) {
      case 'rename':
        if (!this.editNewName.trim()) {
          this.toastrService.warning('请输入新名称', '输入错误');
          return;
        }
        if (this.editNewName === this.selectedMV.name) {
          this.toastrService.warning('新名称与当前名称相同', '输入错误');
          return;
        }
        alterClause = `RENAME ${this.editNewName}`;
        break;
        
      case 'refresh_strategy':
        if (this.editRefreshStrategy === 'MANUAL') {
          alterClause = 'REFRESH MANUAL';
        } else {
          const interval = parseInt(this.editRefreshInterval);
          if (!interval || interval <= 0) {
            this.toastrService.warning('请输入有效的刷新间隔', '输入错误');
            return;
          }
          alterClause = `REFRESH ASYNC EVERY(INTERVAL ${interval} ${this.editRefreshUnit})`;
        }
        break;
        
      case 'properties':
        if (!this.editPropertyKey.trim() || !this.editPropertyValue.trim()) {
          this.toastrService.warning('请输入属性名称和值', '输入错误');
          return;
        }
        // Add session. prefix if it's a session variable
        const key = this.editPropertyKey.startsWith('session.') 
          ? this.editPropertyKey 
          : this.editPropertyKey;
        alterClause = `SET ("${key}" = "${this.editPropertyValue}")`;
        break;
        
      case 'advanced':
        if (!this.editAdvancedClause.trim()) {
          this.toastrService.warning('请输入ALTER子句', '输入错误');
          return;
        }
        alterClause = this.editAdvancedClause;
        break;
    }

    this.editing = true;
    this.mvService
      .alterMaterializedView( this.selectedMV.name, { alter_clause: alterClause })
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: () => {
          this.toastrService.success('物化视图修改成功', '成功');
          this.closeEditDialog();
          this.loadMaterializedViews();
        },
        error: (error) => {
          this.toastrService.danger(
            ErrorHandler.extractErrorMessage(error),
            '修改失败',
          );
          this.editing = false;
        },
      });
  }

  getRefreshTypeBadge(type: string): string {
    const badges = {
      ASYNC: '<span class="badge badge-success">自动</span>',
      MANUAL: '<span class="badge badge-info">手动</span>',
      ROLLUP: '<span class="badge badge-primary">同步</span>',
      INCREMENTAL: '<span class="badge badge-warning">增量</span>',
    };
    return badges[type] || `<span class="badge badge-basic">${type}</span>`;
  }

  getRefreshStateBadge(state: string): string {
    if (!state) return '-';
    const badges = {
      SUCCESS: '<span class="badge badge-success">成功</span>',
      RUNNING: '<span class="badge badge-info">运行中</span>',
      FAILED: '<span class="badge badge-danger">失败</span>',
      PENDING: '<span class="badge badge-warning">等待中</span>',
    };
    return badges[state] || `<span class="badge badge-basic">${state}</span>`;
  }

  formatNumber(num: number): string {
    if (num >= 1000000) {
      return (num / 1000000).toFixed(1) + 'M';
    } else if (num >= 1000) {
      return (num / 1000).toFixed(1) + 'K';
    }
    return num.toString();
  }

  formatSQL(sql: string): void {
    // Simple SQL formatting
    this.createSQL = sql
      .replace(/\bSELECT\b/gi, '\nSELECT')
      .replace(/\bFROM\b/gi, '\nFROM')
      .replace(/\bWHERE\b/gi, '\nWHERE')
      .replace(/\bGROUP BY\b/gi, '\nGROUP BY')
      .replace(/\bORDER BY\b/gi, '\nORDER BY')
      .trim();
  }
}


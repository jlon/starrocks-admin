import { Component, OnInit, OnDestroy, TemplateRef, ViewChild, ChangeDetectorRef } from '@angular/core';
import { NbToastrService, NbDialogService, NbDialogRef } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { NodeService, SqlBlacklistItem } from '../../../../@core/data/node.service';
import { ClusterContextService } from '../../../../@core/data/cluster-context.service';
import { Cluster } from '../../../../@core/data/cluster.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { ConfirmDialogService } from '../../../../@core/services/confirm-dialog.service';

@Component({
  selector: 'ngx-sql-blacklist',
  templateUrl: './sql-blacklist.component.html',
  styleUrls: ['./sql-blacklist.component.scss'],
})
export class SqlBlacklistComponent implements OnInit, OnDestroy {
  blacklistSource: LocalDataSource = new LocalDataSource();
  activeCluster: Cluster | null = null;
  loading = false;
  private destroy$ = new Subject<void>();

  @ViewChild('blacklistDialog', { static: false }) blacklistDialogTemplate!: TemplateRef<any>;
  blacklistDialogRef: NbDialogRef<any> | null = null;
  newBlacklistPattern = '';

  exampleTemplates = [
    {
      title: '禁止 SELECT *',
      description: '防止全表扫描，提升查询性能',
      pattern: 'select\\\\s+\\\\*\\\\s+from',
      icon: 'eye-off-outline'
    },
    {
      title: '禁止 COUNT(*)',
      description: '避免大表计数操作',
      pattern: 'select\\\\s+count\\\\(\\\\*\\\\)\\\\s+from',
      icon: 'hash-outline'
    },
    {
      title: '禁止 DROP 操作',
      description: '防止误删表、库、视图',
      pattern: '.*DROP\\\\s+(TABLE|DATABASE|VIEW).*',
      icon: 'trash-2-outline'
    },
    {
      title: '禁止无条件 DELETE',
      description: '防止误删全表数据',
      pattern: 'DELETE\\\\s+FROM\\\\s+\\\\w+\\\\s*$',
      icon: 'alert-triangle-outline'
    },
    {
      title: '禁止敏感表查询',
      description: '保护特定敏感数据表',
      pattern: '.*FROM\\\\s+sensitive_table.*',
      icon: 'shield-outline'
    },
    {
      title: '禁止大批量 INSERT',
      description: '限制单次插入数据量',
      pattern: 'INSERT\\\\s+INTO.*VALUES\\\\s*\\\\(.*\\\\)\\\\s*,.*\\\\)\\\\s*,.*\\\\)',
      icon: 'download-outline'
    }
  ];

  blacklistSettings = {
    mode: 'external',
    hideSubHeader: true,
    noDataMessage: '暂无 SQL 黑名单规则',
    actions: { add: false, edit: false, delete: true, position: 'right' },
    delete: { deleteButtonContent: '<i class="nb-trash"></i>', confirmDelete: true },
    columns: {
      Id: { title: 'ID', type: 'string', width: '10%' },
      Pattern: { title: '正则表达式', type: 'string', width: '90%' },
    },
  };

  constructor(
    private nodeService: NodeService,
    private toastrService: NbToastrService,
    private clusterContext: ClusterContextService,
    private dialogService: NbDialogService,
    private confirmDialogService: ConfirmDialogService,
    private cdr: ChangeDetectorRef,
  ) {}

  ngOnInit(): void {
    this.clusterContext.activeCluster$.pipe(takeUntil(this.destroy$)).subscribe(cluster => {
      this.activeCluster = cluster;
      if (cluster) {
        this.loadBlacklistIfNotLoading();
      }
    });
  }

  private loadBlacklistIfNotLoading(): void {
    if (!this.loading) {
      this.loadBlacklist();
    }
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadBlacklist(): void {
    this.loading = true;
    this.nodeService.listSqlBlacklist().pipe(takeUntil(this.destroy$)).subscribe({
      next: items => {
        console.log('SQL Blacklist loaded:', items);
        console.log('Loading into table source...');
        this.blacklistSource.load(items);
        console.log('Table source count after load:', this.blacklistSource.count());
        this.loading = false;
      },
      error: error => {
        console.error('SQL Blacklist load error:', error);
        this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '加载失败');
        this.loading = false;
      },
    });
  }

  openAddDialog(): void {
    this.newBlacklistPattern = '';
    this.blacklistDialogRef = this.dialogService.open(this.blacklistDialogTemplate, { closeOnBackdropClick: false, closeOnEsc: true });
  }

  submitPattern(): void {
    const pattern = this.newBlacklistPattern.trim();
    if (!pattern) { this.toastrService.warning('请输入正则表达式', '提示'); return; }
    this.nodeService.addSqlBlacklist(pattern).pipe(takeUntil(this.destroy$)).subscribe({
      next: () => {
        this.toastrService.success('SQL 黑名单规则添加成功', '成功');
        this.blacklistDialogRef?.close();
        this.loadBlacklist();
      },
      error: error => { this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '添加失败'); },
    });
  }

  cancelDialog(): void { this.blacklistDialogRef?.close(); }

  useTemplate(template: any): void {
    this.newBlacklistPattern = template.pattern;
  }

  trackTemplate(index: number, template: any): string {
    return template.title;
  }

  onDeleteConfirm(event: any): void {
    const item = event.data;
    this.confirmDialogService.confirm('确认删除', `确定要删除黑名单规则 #${item.Id} 吗？`, '删除', '取消', 'danger').subscribe(confirmed => {
      if (!confirmed) { event.confirm.reject(); return; }
      this.nodeService.deleteSqlBlacklist(item.Id).pipe(takeUntil(this.destroy$)).subscribe({
        next: () => { this.toastrService.success('SQL 黑名单规则删除成功', '成功'); this.loadBlacklist(); event.confirm.resolve(); },
        error: error => { this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '删除失败'); event.confirm.reject(); },
      });
    });
  }
}

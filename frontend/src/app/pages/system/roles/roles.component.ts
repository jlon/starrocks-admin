import { Component, OnDestroy, OnInit } from '@angular/core';
import { NbDialogService, NbToastrService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Observable, Subject, forkJoin, of } from 'rxjs';
import { finalize, map, switchMap, takeUntil, tap } from 'rxjs/operators';

import {
  CreateRolePayload,
  PermissionDto,
  RoleService,
  RoleSummary,
  UpdateRolePayload,
} from '../../../@core/data/role.service';
import { PermissionService } from '../../../@core/data/permission.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';
import { RolesSystemBadgeCellComponent } from './table/system-badge-cell.component';
import { RolesActionsCellComponent, RoleActionPermissions } from './table/actions-cell.component';
import {
  RoleFormDialogComponent,
  RoleFormDialogResult,
} from './role-form/role-form-dialog.component';

@Component({
  selector: 'ngx-roles',
  templateUrl: './roles.component.html',
  styleUrls: ['./roles.component.scss'],
})
export class RolesComponent implements OnInit, OnDestroy {
  source: LocalDataSource = new LocalDataSource();
  loading = false;
  private destroy$ = new Subject<void>();

  private basePermissions: PermissionDto[] = [];

  hasListPermission = false;
  canCreateRole = false;
  canUpdateRole = false;
  canDeleteRole = false;

  settings = this.buildTableSettings();

  constructor(
    private roleService: RoleService,
    private permissionService: PermissionService,
    private dialogService: NbDialogService,
    private confirmDialog: ConfirmDialogService,
    private toastrService: NbToastrService,
  ) {}

  ngOnInit(): void {
    this.permissionService.permissions$
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => this.applyPermissionState());

    this.applyPermissionState();
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  openCreateRole(): void {
    if (!this.canCreateRole) {
      return;
    }

    this.ensurePermissions().subscribe((permissions) => {
      const dialogRef = this.dialogService.open(RoleFormDialogComponent, {
        context: {
          mode: 'create',
          permissions,
        },
        closeOnBackdropClick: false,
        autoFocus: false,
      });

      dialogRef.onClose.subscribe((result?: RoleFormDialogResult) => {
        if (result && result.mode === 'create') {
          this.createRole(result);
        }
      });
    });
  }

  openEditRole(role: RoleSummary): void {
    if (!this.canUpdateRole || role.is_system) {
      return;
    }

    forkJoin([
      this.ensurePermissions(),
      this.roleService.getRolePermissions(role.id),
    ]).subscribe(([permissions, rolePermissions]) => {
      const assignedIds = new Set(rolePermissions.map((permission) => permission.id));
      const prepared = permissions.map((permission) => ({
        ...permission,
        selected: assignedIds.has(permission.id),
      }));

      const dialogRef = this.dialogService.open(RoleFormDialogComponent, {
        context: {
          mode: 'edit',
          role,
          permissions: prepared,
        },
        closeOnBackdropClick: false,
        autoFocus: false,
      });

      dialogRef.onClose.subscribe((result?: RoleFormDialogResult) => {
        if (result && result.mode === 'edit') {
          this.updateRole(role, result);
        }
      });
    });
  }

  onDeleteRole(role: RoleSummary): void {
    if (!this.canDeleteRole || role.is_system) {
      return;
    }

    this.confirmDialog.confirmDelete(role.name).subscribe((confirmed) => {
      if (!confirmed) {
        return;
      }

      this.roleService.deleteRole(role.id).subscribe({
        next: () => {
          this.toastrService.success('角色已删除', '成功');
          this.loadRoles();
        },
        error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
      });
    });
  }

  private createRole(result: RoleFormDialogResult): void {
    const payload = result.rolePayload as CreateRolePayload;
    this.loading = true;

    this.roleService
      .createRole(payload)
      .pipe(
        switchMap((role) => {
          if (!result.permissionIds.length) {
            return of(role);
          }
          return this.roleService
            .updateRolePermissions(role.id, result.permissionIds)
            .pipe(map(() => role));
        }),
        finalize(() => (this.loading = false)),
      )
      .subscribe({
        next: () => {
          this.toastrService.success('角色创建成功', '成功');
          this.loadRoles();
        },
        error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
      });
  }

  private updateRole(role: RoleSummary, result: RoleFormDialogResult): void {
    const payload = result.rolePayload as UpdateRolePayload;
    this.loading = true;

    this.roleService
      .updateRole(role.id, payload)
      .pipe(
        switchMap(() => this.roleService.updateRolePermissions(role.id, result.permissionIds)),
        finalize(() => (this.loading = false)),
      )
      .subscribe({
        next: () => {
          this.toastrService.success('角色更新成功', '成功');
          this.loadRoles();
        },
        error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
      });
  }

  private applyPermissionState(): void {
    this.hasListPermission = this.permissionService.hasPermission('api:roles:list');
    this.canCreateRole = this.permissionService.hasPermission('api:roles:create');
    this.canUpdateRole = this.permissionService.hasPermission('api:roles:update');
    this.canDeleteRole = this.permissionService.hasPermission('api:roles:delete');

    this.settings = this.buildTableSettings();
    this.settings.columns.actions.onComponentInitFunction = (
      component: RolesActionsCellComponent,
    ) => {
      component.edit.subscribe((role: RoleSummary) => this.openEditRole(role));
      component.remove.subscribe((role: RoleSummary) => this.onDeleteRole(role));
    };

    if ((this.canCreateRole || this.canUpdateRole) && !this.basePermissions.length) {
      this.preloadPermissions();
    }

    this.loadRoles();
  }

  private loadRoles(): void {
    if (!this.hasListPermission) {
      this.source.load([]);
      this.loading = false;
      return;
    }

    this.loading = true;
    this.roleService
      .listRoles()
      .pipe(finalize(() => (this.loading = false)))
      .subscribe({
        next: (roles) => this.source.load(roles),
        error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
      });
  }

  private preloadPermissions(): void {
    this.roleService
      .listPermissions()
      .pipe(tap((permissions) => (this.basePermissions = permissions)))
      .subscribe({
        error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
      });
  }

  private ensurePermissions(): Observable<PermissionDto[]> {
    if (this.basePermissions.length) {
      return of(this.basePermissions.map((permission) => ({ ...permission, selected: false })));
    }

    return this.roleService.listPermissions().pipe(
      tap((permissions) => (this.basePermissions = permissions)),
      map((permissions) => permissions.map((permission) => ({ ...permission, selected: false }))),
    );
  }

  private buildTableSettings(): any {
    return {
      mode: 'external',
      hideSubHeader: false,
      noDataMessage: this.hasListPermission ? '暂无角色数据' : '您暂无查看角色的权限',
      actions: {
        add: false,
        edit: false,
        delete: false,
        position: 'right',
      },
      pager: {
        display: true,
        perPage: 10,
      },
      columns: {
        id: {
          title: 'ID',
          type: 'number',
          width: '8%',
        },
        code: {
          title: '角色代码',
          type: 'string',
        },
        name: {
          title: '角色名称',
          type: 'string',
        },
        description: {
          title: '描述',
          type: 'string',
        },
        is_system: {
          title: '系统角色',
          type: 'custom',
          renderComponent: RolesSystemBadgeCellComponent,
          filter: false,
          sort: false,
        },
        created_at: {
          title: '创建时间',
          type: 'string',
          valuePrepareFunction: (date: string) => new Date(date).toLocaleString('zh-CN'),
        },
        actions: {
          title: '操作',
          type: 'custom',
          renderComponent: RolesActionsCellComponent,
          filter: false,
          sort: false,
          valuePrepareFunction: (cell: unknown, row: RoleSummary): RoleActionPermissions => ({
            canEdit: this.canUpdateRole && !row.is_system,
            canDelete: this.canDeleteRole && !row.is_system,
          }),
        },
      },
    };
  }
}

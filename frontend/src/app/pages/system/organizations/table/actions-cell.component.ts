import { Component, EventEmitter, Input, Output, OnInit, OnDestroy, ChangeDetectorRef } from '@angular/core';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';

import { Organization } from '../../../../@core/data/organization.service';
import { AuthService } from '../../../../@core/data/auth.service';
import { PermissionService } from '../../../../@core/data/permission.service';

@Component({
  selector: 'ngx-organizations-actions-cell',
  template: `
    <div class="actions-container">
      <button
        nbButton
        ghost
        size="tiny"
        status="primary"
        nbTooltip="编辑组织信息"
        nbTooltipPlacement="top"
        [disabled]="!canEdit"
        (click)="onEditClick($event)"
        class="action-btn"
      >
        <nb-icon icon="edit-2-outline"></nb-icon>
      </button>
      <button
        nbButton
        ghost
        size="tiny"
        status="danger"
        [nbTooltip]="rowData?.is_system ? '系统组织无法删除' : '删除组织'"
        nbTooltipPlacement="top"
        [disabled]="!canDelete"
        (click)="onDeleteClick($event)"
        class="action-btn"
      >
        <nb-icon icon="trash-2-outline"></nb-icon>
      </button>
    </div>
  `,
  styles: [
    `
      .actions-container {
        display: flex;
        justify-content: center;
        gap: 0.25rem;
        align-items: center;
      }

      .action-btn {
        transition: all 0.2s ease;
      }

      .action-btn:hover:not(:disabled) {
        transform: translateY(-1px);
      }

      .action-btn:active:not(:disabled) {
        transform: translateY(0);
      }
    `,
  ],
})
export class OrganizationsActionsCellComponent implements OnInit, OnDestroy {
  @Input() rowData!: Organization;
  @Output() edit = new EventEmitter<Organization>();
  @Output() delete = new EventEmitter<Organization>();

  private destroy$ = new Subject<void>();

  constructor(
    private authService: AuthService,
    private permissionService: PermissionService,
    private cdr: ChangeDetectorRef,
  ) {}

  ngOnInit(): void {
    this.permissionService.permissions$
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => {
        this.cdr.markForCheck();
      });

    this.authService.currentUser
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => {
        this.cdr.markForCheck();
      });
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  get canEdit(): boolean {
    if (this.rowData?.is_system) {
      return false;
    }
    if (this.authService.isSuperAdmin()) {
      return true;
    }
    return this.permissionService.hasPermission('api:organizations:update');
  }

  get canDelete(): boolean {
    if (this.rowData?.is_system) {
      return false;
    }
    if (this.authService.isSuperAdmin()) {
      return true;
    }
    return this.permissionService.hasPermission('api:organizations:delete');
  }

  onEditClick(event: Event): void {
    event.stopPropagation();
    if (this.canEdit) {
      this.edit.emit(this.rowData);
    }
  }

  onDeleteClick(event: Event): void {
    event.stopPropagation();
    if (this.canDelete) {
      this.delete.emit(this.rowData);
    }
  }
}

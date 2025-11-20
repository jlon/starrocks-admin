import { Component, EventEmitter, Input, Output, OnInit, OnDestroy, ChangeDetectorRef } from '@angular/core';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';

import { Organization } from '../../../../@core/data/organization.service';
import { AuthService } from '../../../../@core/data/auth.service';
import { PermissionService } from '../../../../@core/data/permission.service';

@Component({
  selector: 'ngx-organizations-actions-cell',
  template: `
    <div class="actions">
      <button
        nbButton
        ghost
        size="tiny"
        status="primary"
        nbTooltip="编辑组织"
        nbTooltipPlacement="top"
        [disabled]="!canEdit"
        (click)="onEditClick($event)"
      >
        <nb-icon icon="edit-2-outline"></nb-icon>
      </button>
      <button
        nbButton
        ghost
        size="tiny"
        status="danger"
        nbTooltip="删除组织"
        nbTooltipPlacement="top"
        [disabled]="!canDelete"
        (click)="onDeleteClick($event)"
      >
        <nb-icon icon="trash-2-outline"></nb-icon>
      </button>
    </div>
  `,
  styles: [
    `
      .actions {
        display: flex;
        justify-content: center;
        gap: var(--nb-space-xs);
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
    // Subscribe to permission changes to trigger change detection
    this.permissionService.permissions$
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => {
        this.cdr.markForCheck();
      });

    // Subscribe to user changes to trigger change detection
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
    // System organizations cannot be edited
    if (this.rowData?.is_system) {
      return false;
    }
    // Check super admin first
    if (this.authService.isSuperAdmin()) {
      return true;
    }
    // Then check specific permission
    return this.permissionService.hasPermission('api:organizations:update');
  }

  get canDelete(): boolean {
    // System organizations cannot be deleted
    if (this.rowData?.is_system) {
      return false;
    }
    // Check super admin first
    if (this.authService.isSuperAdmin()) {
      return true;
    }
    // Then check specific permission
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


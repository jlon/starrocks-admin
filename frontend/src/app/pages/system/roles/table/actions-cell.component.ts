import { Component, EventEmitter, Input, Output } from '@angular/core';

import { RoleSummary } from '../../../../@core/data/role.service';

export interface RoleActionPermissions {
  canEdit: boolean;
  canDelete: boolean;
}

@Component({
  selector: 'ngx-roles-actions-cell',
  template: `
    <div class="actions-container">
      <button
        nbButton
        ghost
        size="tiny"
        status="primary"
        [nbTooltip]="rowData?.is_system ? '系统角色无法编辑' : '编辑角色'"
        nbTooltipPlacement="top"
        [disabled]="!value?.canEdit"
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
        [nbTooltip]="rowData?.is_system ? '系统角色无法删除' : '删除角色'"
        nbTooltipPlacement="top"
        [disabled]="!value?.canDelete"
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

        &:hover:not(:disabled) {
          transform: translateY(-1px);
        }

        &:active:not(:disabled) {
          transform: translateY(0);
        }
      }
    `,
  ],
})
export class RolesActionsCellComponent {
  @Input() value: RoleActionPermissions | null = null;
  @Input() rowData!: RoleSummary;
  @Output() edit = new EventEmitter<RoleSummary>();
  @Output() remove = new EventEmitter<RoleSummary>();

  onEditClick(event: Event): void {
    event.stopPropagation();
    if (this.rowData && !this.rowData.is_system && this.value?.canEdit) {
      this.edit.emit(this.rowData);
    }
  }

  onDeleteClick(event: Event): void {
    event.stopPropagation();
    if (this.rowData && !this.rowData.is_system && this.value?.canDelete) {
      this.remove.emit(this.rowData);
    }
  }
}

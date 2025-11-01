import { Component, EventEmitter, Input, Output } from '@angular/core';

import { RoleSummary } from '../../../../@core/data/role.service';

export interface RoleActionPermissions {
  canEdit: boolean;
  canDelete: boolean;
}

@Component({
  selector: 'ngx-roles-actions-cell',
  template: `
    <div class="actions">
      <button
        nbButton
        ghost
        size="tiny"
        status="primary"
        nbTooltip="编辑角色"
        nbTooltipPlacement="top"
        *ngIf="value?.canEdit"
        (click)="edit.emit(rowData)"
      >
        <nb-icon icon="edit-2-outline"></nb-icon>
      </button>
      <button
        nbButton
        ghost
        size="tiny"
        status="danger"
        nbTooltip="删除角色"
        nbTooltipPlacement="top"
        *ngIf="value?.canDelete"
        (click)="remove.emit(rowData)"
      >
        <nb-icon icon="trash-2-outline"></nb-icon>
      </button>
    </div>
  `,
  styles: [
    `
      .actions {
        display: flex;
        justify-content: flex-end;
        gap: var(--nb-space-xs);
      }
    `,
  ],
})
export class RolesActionsCellComponent {
  @Input() value: RoleActionPermissions | null = null;
  @Input() rowData!: RoleSummary;
  @Output() edit = new EventEmitter<RoleSummary>();
  @Output() remove = new EventEmitter<RoleSummary>();
}

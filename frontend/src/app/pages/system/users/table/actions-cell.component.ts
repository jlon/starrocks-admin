import { Component, EventEmitter, Input, Output } from '@angular/core';

import { UserWithRoles } from '../../../../@core/data/user.service';

@Component({
  selector: 'ngx-users-actions-cell',
  template: `
    <div class="actions">
      <button
        nbButton
        ghost
        size="tiny"
        status="primary"
        *ngIf="value?.canEdit"
        (click)="editUser.emit(rowData)"
        nbTooltip="编辑用户"
        nbTooltipPlacement="top"
      >
        <nb-icon icon="edit-2-outline"></nb-icon>
      </button>

      <button
        nbButton
        ghost
        size="tiny"
        status="danger"
        *ngIf="value?.canDelete"
        (click)="deleteUser.emit(rowData)"
        nbTooltip="删除用户"
        nbTooltipPlacement="top"
      >
        <nb-icon icon="trash-2-outline"></nb-icon>
      </button>
    </div>
  `,
  styles: [
    `
      .actions {
        display: flex;
        gap: var(--nb-space-xs);
        justify-content: center;
      }
    `,
  ],
})
export class UsersActionsCellComponent {
  @Input() value: { canEdit: boolean; canDelete: boolean } | null = null;
  @Input() rowData!: UserWithRoles;
  @Output() editUser = new EventEmitter<UserWithRoles>();
  @Output() deleteUser = new EventEmitter<UserWithRoles>();
}



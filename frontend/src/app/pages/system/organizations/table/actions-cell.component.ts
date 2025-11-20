import { Component, EventEmitter, Input, Output } from '@angular/core';

import { Organization } from '../../../../@core/data/organization.service';

export interface OrganizationActionPermissions {
  canEdit: boolean;
  canDelete: boolean;
}

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
        [disabled]="!value?.canEdit"
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
        [disabled]="!value?.canDelete"
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
        justify-content: flex-end;
        gap: var(--nb-space-xs);
      }
    `,
  ],
})
export class OrganizationsActionsCellComponent {
  @Input() value: OrganizationActionPermissions | null = null;
  @Input() rowData!: Organization;
  @Output() edit = new EventEmitter<Organization>();
  @Output() delete = new EventEmitter<Organization>();

  onEditClick(event: Event): void {
    event.stopPropagation();
    if (this.rowData && !this.rowData.is_system && this.value?.canEdit) {
      this.edit.emit(this.rowData);
    }
  }

  onDeleteClick(event: Event): void {
    event.stopPropagation();
    if (this.rowData && !this.rowData.is_system && this.value?.canDelete) {
      this.delete.emit(this.rowData);
    }
  }
}


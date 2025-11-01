import { Component, EventEmitter, Input, Output } from '@angular/core';

interface RoleRow {
  id: number;
  name: string;
  is_system: boolean;
}

@Component({
  selector: 'ngx-roles-configure-cell',
  template: `
    <button
      nbButton
      size="tiny"
      status="info"
      [disabled]="rowData?.is_system"
      (click)="configure.emit(rowData)"
      nbTooltip="配置权限"
    >
      <nb-icon icon="settings-2-outline"></nb-icon>
      配置权限
    </button>
  `,
})
export class RolesConfigureCellComponent {
  @Input() value: any;
  @Input() rowData: RoleRow;
  @Output() configure = new EventEmitter<RoleRow>();
}



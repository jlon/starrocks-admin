import { Component, Input } from '@angular/core';

@Component({
  selector: 'ngx-roles-system-badge-cell',
  template: `
    <nb-badge
      [text]="value ? '是' : '否'"
      [status]="value ? 'warning' : 'basic'"
      size="small"
    ></nb-badge>
  `,
})
export class RolesSystemBadgeCellComponent {
  @Input() value = false;
}



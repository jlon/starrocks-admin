import { Component, Input } from '@angular/core';

@Component({
  selector: 'ngx-roles-system-badge-cell',
  template: `
    <div class="badge-wrapper">
      <nb-badge
        [text]="value ? '是' : '否'"
        [status]="value ? 'warning' : 'basic'"
        size="small"
      ></nb-badge>
    </div>
  `,
  styles: [
    `
      .badge-wrapper {
        display: flex;
        align-items: center;
        justify-content: center;
        height: 100%;
      }
    `,
  ],
})
export class RolesSystemBadgeCellComponent {
  @Input() value = false;
}



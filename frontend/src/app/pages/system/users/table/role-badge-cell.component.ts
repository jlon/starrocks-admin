import { Component, Input } from '@angular/core';

interface Role {
  id: number;
  name: string;
}

@Component({
  selector: 'ngx-users-role-badge-cell',
  template: `
    <div class="badge-group" *ngIf="value?.length; else empty">
      <nb-badge
        *ngFor="let role of value"
        [text]="role.name"
        status="info"
        size="small"
        class="badge-group__item"
      ></nb-badge>
    </div>
    <ng-template #empty>
      <span class="text-hint">未分配</span>
    </ng-template>
  `,
  styleUrls: ['./role-badge-cell.component.scss'],
})
export class UsersRoleBadgeCellComponent {
  @Input() value: Role[] = [];
}



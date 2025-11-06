import { Component, Input } from '@angular/core';

interface Role {
  id: number;
  name: string;
}

@Component({
  selector: 'ngx-users-role-badge-cell',
  template: `
    <div *ngIf="value?.length; else empty">
      {{ getRoleNames() }}
    </div>
    <ng-template #empty>
      <span class="text-hint">-</span>
    </ng-template>
  `,
  styles: [`
    .text-hint {
      color: #999;
    }
  `],
})
export class UsersRoleBadgeCellComponent {
  @Input() value: Role[] = [];

  getRoleNames(): string {
    return this.value.map(role => role.name).join(', ');
  }
}



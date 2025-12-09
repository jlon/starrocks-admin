import { Component, Input } from '@angular/core';

interface Role {
  id: number;
  name: string;
}

@Component({
  selector: 'ngx-users-role-badge-cell',
  template: `
    <div class="roles-container" *ngIf="value?.length; else empty">
      <span
        class="role-tag"
        *ngFor="let role of getDisplayRoles(); let i = index"
        [class.role-tag--primary]="i === 0"
      >
        {{ role.name }}
      </span>
      <span class="role-more" *ngIf="value.length > maxDisplay">
        +{{ value.length - maxDisplay }}
      </span>
    </div>
    <ng-template #empty>
      <span class="text-hint">-</span>
    </ng-template>
  `,
  styles: [`
    .roles-container {
      display: flex;
      flex-wrap: wrap;
      gap: 0.25rem;
      align-items: center;
    }

    .role-tag {
      display: inline-flex;
      align-items: center;
      padding: 0.1875rem 0.5rem;
      border-radius: 0.25rem;
      font-size: 0.75rem;
      font-weight: 500;
      background: var(--background-basic-color-3);
      color: var(--text-basic-color);
      transition: all 0.15s ease;
    }

    .role-tag--primary {
      background: rgba(var(--color-primary-rgb, 51, 102, 255), 0.12);
      color: var(--color-primary-default);
    }

    .role-more {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      padding: 0.1875rem 0.375rem;
      border-radius: 0.25rem;
      font-size: 0.6875rem;
      font-weight: 600;
      background: var(--background-basic-color-3);
      color: var(--text-hint-color);
    }

    .text-hint {
      color: var(--text-hint-color);
    }
  `],
})
export class UsersRoleBadgeCellComponent {
  @Input() value: Role[] = [];
  maxDisplay = 2;

  getDisplayRoles(): Role[] {
    return this.value.slice(0, this.maxDisplay);
  }
}

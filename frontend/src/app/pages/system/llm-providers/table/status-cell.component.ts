import { Component, Input } from '@angular/core';
import { LLMProvider } from '../../../../@core/data/llm-provider.service';

@Component({
  selector: 'ngx-llm-provider-status-cell',
  template: `
    <div class="status-container">
      <!-- Active status with icon -->
      <div
        class="status-badge"
        [ngClass]="{
          'status-badge--active': rowData.is_active,
          'status-badge--inactive': !rowData.is_active
        }"
      >
        <span class="status-badge__dot"></span>
        <span class="status-badge__text">{{ rowData.is_active ? '已激活' : '未激活' }}</span>
      </div>

      <!-- Enabled/Disabled status -->
      <div
        class="status-tag"
        [ngClass]="{
          'status-tag--enabled': rowData.enabled,
          'status-tag--disabled': !rowData.enabled
        }"
      >
        <nb-icon [icon]="rowData.enabled ? 'checkmark-circle-2-outline' : 'close-circle-outline'"></nb-icon>
        <span>{{ rowData.enabled ? '启用' : '禁用' }}</span>
      </div>
    </div>
  `,
  styles: [
    `
      .status-container {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex-wrap: wrap;
      }

      /* Active status badge with dot indicator */
      .status-badge {
        display: inline-flex;
        align-items: center;
        gap: 0.375rem;
        padding: 0.25rem 0.625rem;
        border-radius: 1rem;
        font-size: 0.75rem;
        font-weight: 500;
        line-height: 1.2;
        transition: all 0.2s ease;
      }

      .status-badge__dot {
        width: 0.5rem;
        height: 0.5rem;
        border-radius: 50%;
        flex-shrink: 0;
      }

      .status-badge--active {
        background: linear-gradient(135deg, rgba(var(--color-success-rgb, 0, 214, 143), 0.15) 0%, rgba(var(--color-success-rgb, 0, 214, 143), 0.08) 100%);
        color: var(--color-success-default);

        .status-badge__dot {
          background: var(--color-success-default);
          box-shadow: 0 0 0 2px rgba(var(--color-success-rgb, 0, 214, 143), 0.2);
          animation: pulse-success 2s infinite;
        }
      }

      .status-badge--inactive {
        background: var(--background-basic-color-3);
        color: var(--text-hint-color);

        .status-badge__dot {
          background: var(--text-hint-color);
          opacity: 0.5;
        }
      }

      /* Enabled/Disabled tag */
      .status-tag {
        display: inline-flex;
        align-items: center;
        gap: 0.25rem;
        padding: 0.1875rem 0.5rem;
        border-radius: 0.25rem;
        font-size: 0.6875rem;
        font-weight: 500;
        text-transform: uppercase;
        letter-spacing: 0.025em;
        transition: all 0.2s ease;

        nb-icon {
          font-size: 0.75rem;
      }
      }

      .status-tag--enabled {
        background: rgba(var(--color-info-rgb, 0, 149, 255), 0.1);
        color: var(--color-info-default);
      }

      .status-tag--disabled {
        background: rgba(var(--color-warning-rgb, 255, 170, 0), 0.1);
        color: var(--color-warning-default);
      }

      /* Pulse animation for active status */
      @keyframes pulse-success {
        0%, 100% {
          box-shadow: 0 0 0 2px rgba(var(--color-success-rgb, 0, 214, 143), 0.2);
        }
        50% {
          box-shadow: 0 0 0 4px rgba(var(--color-success-rgb, 0, 214, 143), 0.1);
        }
      }
    `,
  ],
})
export class LLMProviderStatusCellComponent {
  @Input() rowData!: LLMProvider;
}

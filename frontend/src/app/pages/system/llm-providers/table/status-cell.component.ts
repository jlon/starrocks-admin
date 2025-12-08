import { Component, Input } from '@angular/core';
import { LLMProvider } from '../../../../@core/data/llm-provider.service';

@Component({
  selector: 'ngx-llm-provider-status-cell',
  template: `
    <div class="status-badges">
      <span
        class="badge"
        [ngClass]="{
          'badge-success': rowData.is_active,
          'badge-basic': !rowData.is_active
        }"
      >
        {{ rowData.is_active ? '已激活' : '未激活' }}
      </span>
      <span
        class="badge"
        [ngClass]="{
          'badge-info': rowData.enabled,
          'badge-warning': !rowData.enabled
        }"
      >
        {{ rowData.enabled ? '已启用' : '已禁用' }}
      </span>
    </div>
  `,
  styles: [
    `
      .status-badges {
        display: flex;
        gap: 0.25rem;
        flex-wrap: wrap;
      }
      .badge {
        display: inline-block;
        padding: 0.25em 0.5em;
        font-size: 0.75rem;
        font-weight: 500;
        line-height: 1;
        text-align: center;
        white-space: nowrap;
        vertical-align: baseline;
        border-radius: 0.25rem;
      }
      .badge-success {
        background-color: var(--color-success-default);
        color: var(--color-success-default-contrast);
      }
      .badge-info {
        background-color: var(--color-info-default);
        color: var(--color-info-default-contrast);
      }
      .badge-warning {
        background-color: var(--color-warning-default);
        color: var(--color-warning-default-contrast);
      }
      .badge-basic {
        background-color: var(--background-basic-color-3);
        color: var(--text-basic-color);
      }
    `,
  ],
})
export class LLMProviderStatusCellComponent {
  @Input() rowData!: LLMProvider;
}

import { Component, EventEmitter, Input, Output, OnInit, OnDestroy, ChangeDetectorRef } from '@angular/core';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';

import { LLMProvider } from '../../../../@core/data/llm-provider.service';
import { AuthService } from '../../../../@core/data/auth.service';
import { PermissionService } from '../../../../@core/data/permission.service';

@Component({
  selector: 'ngx-llm-providers-actions-cell',
  template: `
    <div class="actions-container">
      <!-- Primary actions group -->
      <div class="actions-group actions-group--primary">
        <!-- Activate button (only show if not active and enabled) -->
      <button
        *ngIf="!rowData.is_active && rowData.enabled"
        nbButton
          outline
        size="tiny"
        status="success"
          nbTooltip="设为默认提供商"
        nbTooltipPlacement="top"
        [disabled]="!canUpdate"
        (click)="onActivateClick($event)"
          class="action-btn action-btn--activate"
      >
          <nb-icon icon="star-outline"></nb-icon>
          激活
      </button>

      <!-- Test connection -->
      <button
        nbButton
        ghost
        size="tiny"
        status="info"
          nbTooltip="测试 API 连接"
        nbTooltipPlacement="top"
        [disabled]="testingId === rowData.id"
        (click)="onTestClick($event)"
          class="action-btn"
          [class.action-btn--loading]="testingId === rowData.id"
      >
          <nb-icon [icon]="testingId === rowData.id ? 'loader-outline' : 'flash-outline'"
                   [class.spin]="testingId === rowData.id"></nb-icon>
      </button>
      </div>

      <!-- Secondary actions group -->
      <div class="actions-group actions-group--secondary">
      <!-- Toggle enabled -->
      <button
        nbButton
        ghost
        size="tiny"
        [status]="rowData.enabled ? 'warning' : 'success'"
          [nbTooltip]="rowData.enabled ? '暂停使用' : '启用服务'"
        nbTooltipPlacement="top"
        [disabled]="!canUpdate"
        (click)="onToggleClick($event)"
          class="action-btn"
      >
        <nb-icon [icon]="rowData.enabled ? 'pause-circle-outline' : 'play-circle-outline'"></nb-icon>
      </button>

      <!-- Edit -->
      <button
        nbButton
        ghost
        size="tiny"
        status="primary"
          nbTooltip="编辑配置"
        nbTooltipPlacement="top"
        [disabled]="!canUpdate"
        (click)="onEditClick($event)"
          class="action-btn"
      >
        <nb-icon icon="edit-2-outline"></nb-icon>
      </button>

      <!-- Delete -->
      <button
        nbButton
        ghost
        size="tiny"
        status="danger"
          [nbTooltip]="rowData.is_active ? '无法删除已激活的提供商' : '删除提供商'"
        nbTooltipPlacement="top"
        [disabled]="!canDelete || rowData.is_active"
        (click)="onDeleteClick($event)"
          class="action-btn"
      >
        <nb-icon icon="trash-2-outline"></nb-icon>
      </button>
      </div>
    </div>
  `,
  styles: [
    `
      .actions-container {
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.5rem;
      }

      .actions-group {
        display: flex;
        align-items: center;
        gap: 0.125rem;
      }

      .actions-group--primary {
        padding-right: 0.375rem;
        border-right: 1px solid var(--border-basic-color-3);
      }

      .action-btn {
        transition: all 0.2s ease;

        &:hover:not(:disabled) {
          transform: translateY(-1px);
        }

        &:active:not(:disabled) {
          transform: translateY(0);
        }

        &--activate {
          padding-left: 0.5rem;
          padding-right: 0.625rem;

          nb-icon {
            margin-right: 0.25rem;
          }
        }

        &--loading {
          pointer-events: none;
          opacity: 0.7;
        }
      }

      /* Spin animation for loading icon */
      .spin {
        animation: spin 1s linear infinite;
      }

      @keyframes spin {
        from {
          transform: rotate(0deg);
        }
        to {
          transform: rotate(360deg);
        }
      }
    `,
  ],
})
export class LLMProvidersActionsCellComponent implements OnInit, OnDestroy {
  @Input() rowData!: LLMProvider;
  @Input() canUpdate = false;
  @Input() canDelete = false;
  @Input() testingId: number | null = null;

  @Output() edit = new EventEmitter<LLMProvider>();
  @Output() delete = new EventEmitter<LLMProvider>();
  @Output() activate = new EventEmitter<LLMProvider>();
  @Output() toggle = new EventEmitter<LLMProvider>();
  @Output() test = new EventEmitter<LLMProvider>();

  private destroy$ = new Subject<void>();

  constructor(
    private authService: AuthService,
    private permissionService: PermissionService,
    private cdr: ChangeDetectorRef,
  ) {}

  ngOnInit(): void {
    this.permissionService.permissions$
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => {
        this.cdr.markForCheck();
      });

    this.authService.currentUser
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => {
        this.cdr.markForCheck();
      });
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  onActivateClick(event: Event): void {
    event.stopPropagation();
    if (this.canUpdate) {
      this.activate.emit(this.rowData);
    }
  }

  onTestClick(event: Event): void {
    event.stopPropagation();
    this.test.emit(this.rowData);
  }

  onToggleClick(event: Event): void {
    event.stopPropagation();
    if (this.canUpdate) {
      this.toggle.emit(this.rowData);
    }
  }

  onEditClick(event: Event): void {
    event.stopPropagation();
    if (this.canUpdate) {
      this.edit.emit(this.rowData);
    }
  }

  onDeleteClick(event: Event): void {
    event.stopPropagation();
    if (this.canDelete && !this.rowData.is_active) {
      this.delete.emit(this.rowData);
    }
  }
}

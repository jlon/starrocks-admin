import { Component, EventEmitter, Input, Output, OnInit, OnDestroy, ChangeDetectorRef } from '@angular/core';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';

import { LLMProvider } from '../../../../@core/data/llm-provider.service';
import { AuthService } from '../../../../@core/data/auth.service';
import { PermissionService } from '../../../../@core/data/permission.service';

@Component({
  selector: 'ngx-llm-providers-actions-cell',
  template: `
    <div class="actions">
      <!-- Activate button (only show if not active) -->
      <button
        *ngIf="!rowData.is_active && rowData.enabled"
        nbButton
        ghost
        size="tiny"
        status="success"
        nbTooltip="激活此提供商"
        nbTooltipPlacement="top"
        [disabled]="!canUpdate"
        (click)="onActivateClick($event)"
      >
        <nb-icon icon="checkmark-circle-2-outline"></nb-icon>
      </button>

      <!-- Test connection -->
      <button
        nbButton
        ghost
        size="tiny"
        status="info"
        nbTooltip="测试连接"
        nbTooltipPlacement="top"
        [disabled]="testingId === rowData.id"
        (click)="onTestClick($event)"
      >
        <nb-icon [icon]="testingId === rowData.id ? 'loader-outline' : 'flash-outline'"></nb-icon>
      </button>

      <!-- Toggle enabled -->
      <button
        nbButton
        ghost
        size="tiny"
        [status]="rowData.enabled ? 'warning' : 'success'"
        [nbTooltip]="rowData.enabled ? '禁用' : '启用'"
        nbTooltipPlacement="top"
        [disabled]="!canUpdate"
        (click)="onToggleClick($event)"
      >
        <nb-icon [icon]="rowData.enabled ? 'pause-circle-outline' : 'play-circle-outline'"></nb-icon>
      </button>

      <!-- Edit -->
      <button
        nbButton
        ghost
        size="tiny"
        status="primary"
        nbTooltip="编辑"
        nbTooltipPlacement="top"
        [disabled]="!canUpdate"
        (click)="onEditClick($event)"
      >
        <nb-icon icon="edit-2-outline"></nb-icon>
      </button>

      <!-- Delete -->
      <button
        nbButton
        ghost
        size="tiny"
        status="danger"
        nbTooltip="删除"
        nbTooltipPlacement="top"
        [disabled]="!canDelete || rowData.is_active"
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
        justify-content: center;
        gap: 0.25rem;
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

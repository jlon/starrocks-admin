import { Component, OnDestroy, OnInit } from '@angular/core';
import { NbDialogService, NbToastrService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { TranslateService } from '@ngx-translate/core';

import {
  LLMProvider,
  LLMProviderService,
  CreateLLMProviderRequest,
  UpdateLLMProviderRequest,
} from '../../../@core/data/llm-provider.service';
import { PermissionService } from '../../../@core/data/permission.service';
import { ErrorHandler } from '../../../@core/utils/error-handler';
import { ConfirmDialogService } from '../../../@core/services/confirm-dialog.service';
import { LLMProvidersActionsCellComponent } from './table/actions-cell.component';
import { LLMProviderStatusCellComponent } from './table/status-cell.component';
import {
  LLMProviderFormDialogComponent,
  LLMProviderFormDialogResult,
} from './llm-provider-form/llm-provider-form-dialog.component';
import { AuthService } from '../../../@core/data/auth.service';

@Component({
  selector: 'ngx-llm-providers',
  templateUrl: './llm-providers.component.html',
  styleUrls: ['./llm-providers.component.scss'],
})
export class LLMProvidersComponent implements OnInit, OnDestroy {
  source: LocalDataSource = new LocalDataSource();
  loading = false;
  testingId: number | null = null;
  private destroy$ = new Subject<void>();

  isSuperAdmin = false;
  hasListPermission = false;
  canCreate = false;
  canUpdate = false;
  canDelete = false;

  settings = this.buildTableSettings();

  constructor(
    private llmService: LLMProviderService,
    private permissionService: PermissionService,
    private dialogService: NbDialogService,
    private confirmDialog: ConfirmDialogService,
    private toastrService: NbToastrService,
    private authService: AuthService,
    private translateService: TranslateService,
  ) {}

  ngOnInit(): void {
    this.permissionService.permissions$
      .pipe(takeUntil(this.destroy$))
      .subscribe(() => this.applyPermissionState());

    this.applyPermissionState();
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }

  loadProviders(): void {
    if (!this.hasListPermission) {
      this.loading = false;
      this.source.load([]);
      return;
    }

    this.loading = true;
    this.llmService.listProviders().subscribe({
      next: (providers) => {
        this.source.load(providers);
        this.loading = false;
      },
      error: (error) => {
        ErrorHandler.handleHttpError(error, this.toastrService);
        this.loading = false;
      },
    });
  }

  openCreateProvider(): void {
    if (!this.canCreate) return;

    const dialogRef = this.dialogService.open(LLMProviderFormDialogComponent, {
      context: { mode: 'create' },
      closeOnBackdropClick: false,
      autoFocus: false,
    });

    dialogRef.onClose.subscribe((result?: LLMProviderFormDialogResult) => {
      if (result) this.createProvider(result);
    });
  }

  openEditProvider(provider: LLMProvider): void {
    if (!this.canUpdate) return;

    const dialogRef = this.dialogService.open(LLMProviderFormDialogComponent, {
      context: { mode: 'edit', provider },
      closeOnBackdropClick: false,
      autoFocus: false,
    });

    dialogRef.onClose.subscribe((result?: LLMProviderFormDialogResult) => {
      if (result) this.updateProvider(provider.id, result);
    });
  }

  deleteProvider(provider: LLMProvider): void {
    if (!this.canDelete) return;

    this.translateService.get('llm_providers.toastr.delete_confirm_message').subscribe((message) => {
      this.confirmDialog
        .confirmDelete(provider.display_name, message)
        .subscribe((confirmed) => {
          if (confirmed) this.performDelete(provider.id);
        });
    });
  }

  activateProvider(provider: LLMProvider): void {
    if (!this.canUpdate) return;

    this.loading = true;
    this.llmService.activateProvider(provider.id).subscribe({
      next: () => {
        this.translateService.get([
          'llm_providers.toastr.activated',
          'common.success'
        ], { name: provider.display_name }).subscribe((translations) => {
          this.toastrService.success(
            translations['llm_providers.toastr.activated'],
            translations['common.success']
          );
        });
        this.loadProviders();
      },
      error: (error) => {
        ErrorHandler.handleHttpError(error, this.toastrService);
        this.loading = false;
      },
    });
  }

  toggleEnabled(provider: LLMProvider): void {
    if (!this.canUpdate) return;

    const newEnabled = !provider.enabled;
    this.loading = true;
    this.llmService.updateProvider(provider.id, { enabled: newEnabled }).subscribe({
      next: () => {
        const messageKey = newEnabled ? 'llm_providers.toastr.enabled' : 'llm_providers.toastr.disabled';
        this.translateService.get([
          messageKey,
          'common.success'
        ], { name: provider.display_name }).subscribe((translations) => {
          this.toastrService.success(
            translations[messageKey],
            translations['common.success']
          );
        });
        this.loadProviders();
      },
      error: (error) => {
        ErrorHandler.handleHttpError(error, this.toastrService);
        this.loading = false;
      },
    });
  }

  testConnection(provider: LLMProvider): void {
    this.testingId = provider.id;
    this.llmService.testConnection(provider.id).subscribe({
      next: (result) => {
        if (result.success) {
          this.translateService.get([
            'llm_providers.toastr.test_success',
            'llm_providers.toastr.test_success_title'
          ], { latency: result.latency_ms }).subscribe((translations) => {
            this.toastrService.success(
              translations['llm_providers.toastr.test_success'],
              translations['llm_providers.toastr.test_success_title']
            );
          });
        } else {
          this.translateService.get('llm_providers.toastr.test_failed_title').subscribe((title) => {
            this.toastrService.warning(result.message, title);
          });
        }
        this.testingId = null;
      },
      error: (error) => {
        ErrorHandler.handleHttpError(error, this.toastrService);
        this.testingId = null;
      },
    });
  }

  private createProvider(result: LLMProviderFormDialogResult): void {
    const payload: CreateLLMProviderRequest = {
      name: result.name,
      display_name: result.display_name,
      api_base: result.api_base,
      model_name: result.model_name,
      api_key: result.api_key!,
      max_tokens: result.max_tokens,
      temperature: result.temperature,
      timeout_seconds: result.timeout_seconds,
      priority: result.priority,
    };

    this.loading = true;
    this.llmService.createProvider(payload).subscribe({
      next: () => {
        this.translateService.get([
          'llm_providers.toastr.create_success',
          'common.success'
        ]).subscribe((translations) => {
          this.toastrService.success(
            translations['llm_providers.toastr.create_success'],
            translations['common.success']
          );
        });
        this.loadProviders();
      },
      error: (error) => {
        ErrorHandler.handleHttpError(error, this.toastrService);
        this.loading = false;
      },
    });
  }

  private updateProvider(id: number, result: LLMProviderFormDialogResult): void {
    const payload: UpdateLLMProviderRequest = {
      display_name: result.display_name,
      api_base: result.api_base,
      model_name: result.model_name,
      max_tokens: result.max_tokens,
      temperature: result.temperature,
      timeout_seconds: result.timeout_seconds,
      priority: result.priority,
    };

    // Only include api_key if it was changed
    if (result.api_key) {
      payload.api_key = result.api_key;
    }

    this.loading = true;
    this.llmService.updateProvider(id, payload).subscribe({
      next: () => {
        this.translateService.get([
          'llm_providers.toastr.update_success',
          'common.success'
        ]).subscribe((translations) => {
          this.toastrService.success(
            translations['llm_providers.toastr.update_success'],
            translations['common.success']
          );
        });
        this.loadProviders();
      },
      error: (error) => {
        ErrorHandler.handleHttpError(error, this.toastrService);
        this.loading = false;
      },
    });
  }

  private performDelete(id: number): void {
    this.loading = true;
    this.llmService.deleteProvider(id).subscribe({
      next: () => {
        this.translateService.get([
          'llm_providers.toastr.delete_success',
          'common.success'
        ]).subscribe((translations) => {
          this.toastrService.success(
            translations['llm_providers.toastr.delete_success'],
            translations['common.success']
          );
        });
        this.loadProviders();
      },
      error: (error) => {
        ErrorHandler.handleHttpError(error, this.toastrService);
        this.loading = false;
      },
    });
  }

  private applyPermissionState(): void {
    this.isSuperAdmin = this.authService.isSuperAdmin();
    this.hasListPermission =
      this.permissionService.hasPermission('api:llm:providers:list') || this.isSuperAdmin;
    this.canCreate =
      this.permissionService.hasPermission('api:llm:providers:create') || this.isSuperAdmin;
    this.canUpdate =
      this.permissionService.hasPermission('api:llm:providers:update') || this.isSuperAdmin;
    this.canDelete =
      this.permissionService.hasPermission('api:llm:providers:delete') || this.isSuperAdmin;

    this.settings = this.buildTableSettings();

    if (this.hasListPermission) {
      this.loadProviders();
    }
  }

  private buildTableSettings(): any {
    return {
      actions: {
        add: false,
        edit: false,
        delete: false,
        position: 'right',
      },
      columns: {
        display_name: {
          title: this.translateService.instant('llm_providers.table.display_name'),
          type: 'string',
          width: '15%',
        },
        name: {
          title: this.translateService.instant('llm_providers.table.name'),
          type: 'string',
          width: '10%',
        },
        model_name: {
          title: this.translateService.instant('llm_providers.table.model_name'),
          type: 'string',
          width: '15%',
        },
        api_base: {
          title: this.translateService.instant('llm_providers.table.api_base'),
          type: 'string',
          width: '20%',
          valuePrepareFunction: (cell: string) => {
            // Truncate long URLs
            return cell.length > 40 ? cell.substring(0, 40) + '...' : cell;
          },
        },
        status: {
          title: this.translateService.instant('llm_providers.table.status'),
          type: 'custom',
          width: '15%',
          filter: false,
          renderComponent: LLMProviderStatusCellComponent,
        },
        priority: {
          title: this.translateService.instant('llm_providers.table.priority'),
          type: 'number',
          width: '8%',
        },
        actions: {
          title: this.translateService.instant('llm_providers.table.actions'),
          type: 'custom',
          width: '17%',
          filter: false,
          sort: false,
          renderComponent: LLMProvidersActionsCellComponent,
          onComponentInitFunction: (instance: LLMProvidersActionsCellComponent) => {
            instance.canUpdate = this.canUpdate;
            instance.canDelete = this.canDelete;
            instance.testingId = this.testingId;
            instance.edit.subscribe((provider) => this.openEditProvider(provider));
            instance.delete.subscribe((provider) => this.deleteProvider(provider));
            instance.activate.subscribe((provider) => this.activateProvider(provider));
            instance.toggle.subscribe((provider) => this.toggleEnabled(provider));
            instance.test.subscribe((provider) => this.testConnection(provider));
          },
        },
      },
    };
  }
}

import { NgModule } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule, ReactiveFormsModule } from '@angular/forms';

import {
  NbCardModule,
  NbButtonModule,
  NbInputModule,
  NbSelectModule,
  NbCheckboxModule,
  NbSpinnerModule,
  NbAlertModule,
  NbTabsetModule,
  NbAccordionModule,
  NbIconModule,
  NbDialogModule,
  NbToastrModule,
  NbListModule,
  NbBadgeModule,
  NbTooltipModule,
  NbFormFieldModule,
  NbUserModule,
  NbRadioModule,
} from '@nebular/theme';

import { Ng2SmartTableModule } from 'ng2-smart-table';
import { ThemeModule } from '../../@theme/theme.module';

import { SystemRoutingModule } from './system-routing.module';
import { UsersComponent } from './users/users.component';
import { UsersRoleBadgeCellComponent } from './users/table/role-badge-cell.component';
import { UsersActionsCellComponent } from './users/table/actions-cell.component';
import { RolesComponent } from './roles/roles.component';
import { RolesSystemBadgeCellComponent } from './roles/table/system-badge-cell.component';
import { PermissionTreeComponent } from './roles/permission-tree/permission-tree.component';
import { HasPermissionDirective } from '../../@core/directives/has-permission.directive';
import { UserFormDialogComponent } from './users/user-form/user-form-dialog.component';
import { RoleFormDialogComponent } from './roles/role-form/role-form-dialog.component';
import { RolesActionsCellComponent } from './roles/table/actions-cell.component';
import { OrganizationsComponent } from './organizations/organizations.component';
import { OrganizationsActionsCellComponent } from './organizations/table/actions-cell.component';
import { OrganizationFormDialogComponent } from './organizations/organization-form/organization-form-dialog.component';
import { LLMProvidersComponent } from './llm-providers/llm-providers.component';
import { LLMProvidersActionsCellComponent } from './llm-providers/table/actions-cell.component';
import { LLMProviderStatusCellComponent } from './llm-providers/table/status-cell.component';
import { LLMProviderFormDialogComponent } from './llm-providers/llm-provider-form/llm-provider-form-dialog.component';

@NgModule({
  declarations: [
    UsersComponent,
    UsersRoleBadgeCellComponent,
    UsersActionsCellComponent,
    RolesComponent,
    RolesSystemBadgeCellComponent,
    RolesActionsCellComponent,
    PermissionTreeComponent,
    HasPermissionDirective,
    UserFormDialogComponent,
    RoleFormDialogComponent,
    OrganizationsComponent,
    OrganizationsActionsCellComponent,
    OrganizationFormDialogComponent,
    LLMProvidersComponent,
    LLMProvidersActionsCellComponent,
    LLMProviderStatusCellComponent,
    LLMProviderFormDialogComponent,
  ],
  imports: [
    CommonModule,
    FormsModule,
    ReactiveFormsModule,
    SystemRoutingModule,
    ThemeModule,
    NbCardModule,
    NbButtonModule,
    NbInputModule,
    NbSelectModule,
    NbCheckboxModule,
    NbSpinnerModule,
    NbAlertModule,
    NbTabsetModule,
    NbAccordionModule,
    NbIconModule,
    NbDialogModule,
    NbToastrModule,
    NbListModule,
    NbBadgeModule,
    NbTooltipModule,
    NbFormFieldModule,
    NbUserModule,
    NbRadioModule,
    Ng2SmartTableModule,
  ],
})
export class SystemModule {}


import { NgModule } from '@angular/core';
import { RouterModule, Routes } from '@angular/router';
import { PermissionGuard } from '../../@core/guards/permission.guard';

import { UsersComponent } from './users/users.component';
import { RolesComponent } from './roles/roles.component';
import { OrganizationsComponent } from './organizations/organizations.component';
import { LLMProvidersComponent } from './llm-providers/llm-providers.component';

const routes: Routes = [
  {
    path: '',
    redirectTo: 'users',
    pathMatch: 'full',
  },
  {
    path: 'users',
    component: UsersComponent,
    data: { permission: 'menu:system:users' },
    canActivate: [PermissionGuard],
  },
  {
    path: 'roles',
    component: RolesComponent,
    data: { permission: 'menu:system:roles' },
    canActivate: [PermissionGuard],
  },
  {
    path: 'organizations',
    component: OrganizationsComponent,
    data: { permission: 'menu:system:organizations' },
    canActivate: [PermissionGuard],
  },
  {
    path: 'llm-providers',
    component: LLMProvidersComponent,
    data: { permission: 'menu:system:llm' },
    canActivate: [PermissionGuard],
  },
];

@NgModule({
  imports: [RouterModule.forChild(routes)],
  exports: [RouterModule],
})
export class SystemRoutingModule {}


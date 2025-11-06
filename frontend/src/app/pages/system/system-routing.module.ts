import { NgModule } from '@angular/core';
import { RouterModule, Routes } from '@angular/router';
import { PermissionGuard } from '../../@core/guards/permission.guard';

import { UsersComponent } from './users/users.component';
import { RolesComponent } from './roles/roles.component';

const routes: Routes = [
  {
    path: '',
    redirectTo: 'users',
    pathMatch: 'full',
  },
  {
    path: 'users',
    component: UsersComponent,
    data: { permission: 'menu:users' },
    canActivate: [PermissionGuard],
  },
  {
    path: 'roles',
    component: RolesComponent,
    data: { permission: 'menu:roles' },
    canActivate: [PermissionGuard],
  },
];

@NgModule({
  imports: [RouterModule.forChild(routes)],
  exports: [RouterModule],
})
export class SystemRoutingModule {}


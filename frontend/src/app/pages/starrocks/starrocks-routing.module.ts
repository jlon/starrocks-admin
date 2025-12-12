import { NgModule } from '@angular/core';
import { RouterModule, Routes } from '@angular/router';
import { DashboardComponent } from './dashboard/dashboard.component';
import { ClusterListComponent } from './clusters/cluster-list/cluster-list.component';
import { ClusterFormComponent } from './clusters/cluster-form/cluster-form.component';
import { ClusterDetailComponent } from './clusters/cluster-detail/cluster-detail.component';
import { BackendsComponent } from './backends/backends.component';
import { FrontendsComponent } from './frontends/frontends.component';
import { MaterializedViewsComponent } from './materialized-views/materialized-views.component';
import { QueryExecutionComponent } from './queries/query-execution/query-execution.component';
import { ProfileQueriesComponent } from './queries/profile-queries/profile-queries.component';
import { AuditLogsComponent } from './queries/audit-logs/audit-logs.component';
import { SqlBlacklistComponent } from './queries/sql-blacklist/sql-blacklist.component';
import { ClusterOverviewComponent } from './cluster-overview/cluster-overview.component';
import { SessionsComponent } from './sessions/sessions.component';
import { VariablesComponent } from './variables/variables.component';
import { SystemManagementComponent } from './system-management/system-management.component';
import { PermissionGuard } from '../../@core/guards/permission.guard';

const routes: Routes = [
  {
    path: '',
    redirectTo: 'dashboard',
    pathMatch: 'full',
  },
  {
    path: 'dashboard',
    component: DashboardComponent,
    data: { reuse: true },
  },
  {
    path: 'clusters',
    children: [
      {
        path: '',
        component: ClusterListComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'api:clusters:list', reuse: true },
      },
      {
        path: 'new',
        component: ClusterFormComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'api:clusters:create', reuse: true },
      },
      {
        path: ':id',
        component: ClusterDetailComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'api:clusters:get', reuse: true },
      },
      {
        path: ':id/edit',
        component: ClusterFormComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'api:clusters:update', reuse: true },
      },
    ],
  },
  {
    path: 'backends',
    component: BackendsComponent,
    canActivate: [PermissionGuard],
    data: { permission: 'api:clusters:backends', reuse: true },
  },
  {
    path: 'frontends',
    component: FrontendsComponent,
    canActivate: [PermissionGuard],
    data: { permission: 'api:clusters:frontends', reuse: true },
  },
  {
    path: 'materialized-views',
    component: MaterializedViewsComponent,
    canActivate: [PermissionGuard],
    data: { permission: 'api:clusters:materialized_views', reuse: true },
  },
  {
    path: 'queries',
    children: [
      {
        path: '',
        redirectTo: 'execution',
        pathMatch: 'full',
      },
      {
        path: 'execution',
        component: QueryExecutionComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'api:clusters:queries', reuse: true },
      },
      {
        path: 'profiles',
        component: ProfileQueriesComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'menu:queries:profiles', reuse: true },
      },
      {
        path: 'audit-logs',
        component: AuditLogsComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'menu:queries:audit-logs', reuse: true },
      },
      {
        path: 'blacklist',
        component: SqlBlacklistComponent,
        canActivate: [PermissionGuard],
        data: { permission: 'menu:queries:blacklist', reuse: true },
      },
    ],
  },
  {
    path: 'sessions',
    component: SessionsComponent,
    canActivate: [PermissionGuard],
    data: { permission: 'api:clusters:sessions', reuse: true },
  },
  {
    path: 'variables',
    component: VariablesComponent,
    canActivate: [PermissionGuard],
    data: { permission: 'api:clusters:variables', reuse: true },
  },
  {
    path: 'system',
    component: SystemManagementComponent,
    canActivate: [PermissionGuard],
    data: { permission: 'menu:system-functions', reuse: true },
  },
  {
    path: 'overview',
    component: ClusterOverviewComponent,
    canActivate: [PermissionGuard],
    data: { permission: 'menu:overview', reuse: true },
  },
];

@NgModule({
  imports: [RouterModule.forChild(routes)],
  exports: [RouterModule],
})
export class StarRocksRoutingModule {}

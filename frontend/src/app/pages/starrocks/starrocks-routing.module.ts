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
import { ClusterOverviewComponent } from './cluster-overview/cluster-overview.component';
import { SessionsComponent } from './sessions/sessions.component';
import { VariablesComponent } from './variables/variables.component';
import { SystemManagementComponent } from './system-management/system-management.component';

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
        data: { reuse: true },
      },
      {
        path: 'new',
        component: ClusterFormComponent,
        data: { reuse: true },
      },
      {
        path: ':id',
        component: ClusterDetailComponent,
        data: { reuse: true },
      },
      {
        path: ':id/edit',
        component: ClusterFormComponent,
        data: { reuse: true },
      },
    ],
  },
  {
    path: 'backends',
    component: BackendsComponent,
    data: { reuse: true },
  },
  {
    path: 'frontends',
    component: FrontendsComponent,
    data: { reuse: true },
  },
  {
    path: 'materialized-views',
    component: MaterializedViewsComponent,
    data: { reuse: true },
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
        data: { reuse: true },
      },
      {
        path: 'profiles',
        component: ProfileQueriesComponent,
        data: { reuse: true },
      },
      {
        path: 'audit-logs',
        component: AuditLogsComponent,
        data: { reuse: true },
      },
    ],
  },
  {
    path: 'sessions',
    component: SessionsComponent,
    data: { reuse: true },
  },
  {
    path: 'variables',
    component: VariablesComponent,
    data: { reuse: true },
  },
  {
    path: 'system',
    component: SystemManagementComponent,
    data: { reuse: true },
  },
  {
    path: 'overview',
    component: ClusterOverviewComponent,
    data: { reuse: true },
  },
];

@NgModule({
  imports: [RouterModule.forChild(routes)],
  exports: [RouterModule],
})
export class StarRocksRoutingModule {}


import { NgModule, NO_ERRORS_SCHEMA } from '@angular/core';
import { FormsModule, ReactiveFormsModule } from '@angular/forms';
import {
  NbCardModule,
  NbButtonModule,
  NbInputModule,
  NbSelectModule,
  NbFormFieldModule,
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
  NbProgressBarModule,
  NbToggleModule,
  NbTooltipModule,
  NbTagModule,
} from '@nebular/theme';
import { Ng2SmartTableModule } from 'ng2-smart-table';
import { NgxEchartsModule } from 'ngx-echarts';
import { DragDropModule } from '@angular/cdk/drag-drop';
import { ThemeModule } from '../../@theme/theme.module';

import { StarRocksRoutingModule } from './starrocks-routing.module';
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
import { SessionsComponent } from './sessions/sessions.component';
import { VariablesComponent } from './variables/variables.component';
import { SystemManagementComponent } from './system-management/system-management.component';
import { ClusterOverviewComponent } from './cluster-overview/cluster-overview.component';
import { MetricCardGroupComponent } from './cluster-overview/metric-card-group/metric-card-group.component';
import { NestedLinkRenderComponent } from './system-management/nested-link-render.component';
import { AddFunctionDialogComponent } from './system-management/add-function-dialog/add-function-dialog.component';
import { EditFunctionDialogComponent } from './system-management/edit-function-dialog/edit-function-dialog.component';
import { ConfirmDialogComponent } from '../../@core/components/confirm-dialog/confirm-dialog.component';
import { ActiveToggleRenderComponent } from './materialized-views/active-toggle-render.component';

@NgModule({
  declarations: [
    DashboardComponent,
    ClusterListComponent,
    ClusterFormComponent,
    ClusterDetailComponent,
    BackendsComponent,
    FrontendsComponent,
    MaterializedViewsComponent,
    ActiveToggleRenderComponent,
    QueryExecutionComponent,
    ProfileQueriesComponent,
    AuditLogsComponent,
    SqlBlacklistComponent,
    SessionsComponent,
    VariablesComponent,
    SystemManagementComponent,
    ClusterOverviewComponent,
    MetricCardGroupComponent,
    NestedLinkRenderComponent,
    AddFunctionDialogComponent,
    EditFunctionDialogComponent,
    ConfirmDialogComponent,
  ],
  imports: [
    FormsModule,
    ReactiveFormsModule,
    StarRocksRoutingModule,
    ThemeModule,
    NbCardModule,
    NbButtonModule,
    NbInputModule,
    NbSelectModule,
    NbFormFieldModule,
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
    NbProgressBarModule,
    NbToggleModule,
    NbTooltipModule,
    NbTagModule,
    Ng2SmartTableModule,
    NgxEchartsModule,
    DragDropModule,
  ],
  schemas: [NO_ERRORS_SCHEMA],
})
export class StarRocksModule {}


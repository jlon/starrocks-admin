import { Component, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { ActivatedRoute, Router } from '@angular/router';
import { NbToastrService } from '@nebular/theme';
import { ClusterService, Cluster } from '../../../../@core/data/cluster.service';
import { OrganizationService, Organization } from '../../../../@core/data/organization.service';
import { AuthService } from '../../../../@core/data/auth.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { TabReuseService } from '../../../../@core/services/tab-reuse.service';

@Component({
  selector: 'ngx-cluster-form',
  templateUrl: './cluster-form.component.html',
  styleUrls: ['./cluster-form.component.scss'],
})
export class ClusterFormComponent implements OnInit {
  clusterForm: FormGroup;
  loading = false;
  isEditMode = false;
  clusterId: number | null = null;
  connectionTested = false; // Track if connection has been tested
  connectionValid = false;  // Track if connection is valid
  
  // Organization support
  organizations: Organization[] = [];
  currentOrganization?: Organization;
  isSuperAdmin = false;
  organizationsLoading = false;

  constructor(
    private fb: FormBuilder,
    private clusterService: ClusterService,
    private organizationService: OrganizationService,
    private authService: AuthService,
    private router: Router,
    private route: ActivatedRoute,
    private toastrService: NbToastrService,
    private tabReuseService: TabReuseService,
  ) {
    this.clusterForm = this.fb.group({
      organization_id: [null],
      name: ['', [Validators.required, Validators.maxLength(100)]],
      description: [''],
      fe_host: ['', [Validators.required]],
      fe_http_port: [8030, [Validators.required, Validators.min(1), Validators.max(65535)]],
      fe_query_port: [9030, [Validators.required, Validators.min(1), Validators.max(65535)]],
      username: ['root', [Validators.required]],
      password: ['', [Validators.required]],
      enable_ssl: [false],
      connection_timeout: [10, [Validators.min(1), Validators.max(300)]],
      catalog: ['default_catalog'],
      deployment_mode: ['shared_nothing', [Validators.required]],
      tags: [''],
    });
  }

  ngOnInit(): void {
    // Determine if current user is super admin
    this.isSuperAdmin = this.authService.isSuperAdmin();
    
    // Load organizations and current organization
    this.loadOrganizationData();
    
    const id = this.route.snapshot.paramMap.get('id');
    if (id && id !== 'new') {
      this.isEditMode = true;
      this.clusterId = parseInt(id, 10);
      this.loadCluster();
    }
  }
  
  private loadOrganizationData(): void {
    // Load organizations for super admin
    if (this.isSuperAdmin) {
      this.organizationsLoading = true;
      this.organizationService.listOrganizations().subscribe({
        next: (orgs) => {
          this.organizations = orgs;
          this.organizationsLoading = false;
          // Set organization_id as required for super admin
          this.clusterForm.get('organization_id')?.setValidators([Validators.required]);
          this.clusterForm.get('organization_id')?.updateValueAndValidity();
        },
        error: (error) => {
          ErrorHandler.handleHttpError(error, this.toastrService);
          this.organizationsLoading = false;
        },
      });
    } else {
      // Load current organization for org admin
      const currentUser = this.authService.currentUserValue;
      if (currentUser?.organization_id) {
        this.organizationService.getOrganization(currentUser.organization_id).subscribe({
          next: (org) => {
            this.currentOrganization = org;
            // Auto-set organization for org admin
            this.clusterForm.get('organization_id')?.setValue(org.id);
            this.clusterForm.get('organization_id')?.disable();
          },
          error: (error) => ErrorHandler.handleHttpError(error, this.toastrService),
        });
      }
    }
  }

  loadCluster(): void {
    if (!this.clusterId) return;

    this.loading = true;
    this.clusterService.getCluster(this.clusterId).subscribe({
      next: (cluster) => {
        this.clusterForm.patchValue({
          organization_id: cluster.organization_id,
          name: cluster.name,
          description: cluster.description,
          fe_host: cluster.fe_host,
          fe_http_port: cluster.fe_http_port,
          fe_query_port: cluster.fe_query_port,
          username: cluster.username,
          enable_ssl: cluster.enable_ssl,
          connection_timeout: cluster.connection_timeout,
          catalog: cluster.catalog,
          deployment_mode: cluster.deployment_mode,
          tags: cluster.tags.join(', '),
        });
        // Password is not loaded for security
        this.clusterForm.get('password')?.clearValidators();
        this.clusterForm.get('password')?.updateValueAndValidity();
        this.loading = false;
      },
      error: (error) => {
        this.toastrService.danger(
          ErrorHandler.extractErrorMessage(error),
          '错误',
        );
        this.loading = false;
      },
    });
  }

  onSubmit(): void {
    if (this.clusterForm.invalid) {
      Object.keys(this.clusterForm.controls).forEach(key => {
        this.clusterForm.get(key)?.markAsTouched();
      });
      return;
    }

    this.loading = true;
    const formValue = this.clusterForm.value;
    
    // Parse tags
    const tags = formValue.tags
      ? formValue.tags.split(',').map((t: string) => t.trim()).filter((t: string) => t)
      : [];

    const clusterData = {
      ...formValue,
      tags,
      organization_id: this.clusterForm.get('organization_id')?.value,
    };

    // Remove password if in edit mode and password is empty
    if (this.isEditMode && !formValue.password) {
      delete clusterData.password;
    }

    const request$ = this.isEditMode && this.clusterId
      ? this.clusterService.updateCluster(this.clusterId, clusterData)
      : this.clusterService.createCluster(clusterData);

    request$.subscribe({
      next: (cluster) => {
        // For new cluster, test connection after creation
        if (!this.isEditMode && cluster.id) {
          this.testConnectionAfterCreate(cluster.id);
        } else {
          this.toastrService.success('集群更新成功', '成功');
          this.navigateToDashboardWithRefresh();
        }
      },
      error: (error) => {
        this.toastrService.danger(
          ErrorHandler.extractErrorMessage(error),
          '错误',
        );
        this.loading = false;
      },
    });
  }

  private testConnectionAfterCreate(clusterId: number): void {
    this.clusterService.getHealth(clusterId).subscribe({
      next: (health) => {
        if (health.status === 'healthy') {
          this.toastrService.success('集群创建成功，健康检查通过', '成功');
        } else if (health.status === 'warning') {
          this.toastrService.warning('集群已创建，但健康检查发现问题。请检查配置', '警告');
        } else {
          this.toastrService.warning('集群已创建，但健康检查失败。请检查配置', '警告');
        }
        this.navigateToDashboardWithRefresh();
      },
      error: () => {
        this.toastrService.warning('集群已创建，但健康检查失败。请检查配置', '警告');
        this.navigateToDashboardWithRefresh();
      },
    });
  }

  onCancel(): void {
    this.router.navigate(['/pages/starrocks/dashboard']);
  }

  testConnection(): void {
    // Check required fields for new cluster
    if (!this.isEditMode) {
      const requiredFields = ['fe_host', 'fe_http_port', 'fe_query_port', 'username', 'password'];
      const missingFields = requiredFields.filter(field => !this.clusterForm.get(field)?.value);
      
      if (missingFields.length > 0) {
        this.toastrService.warning('请先填写完整的连接信息（FE地址、端口、用户名、密码）', '提示');
        return;
      }
    }

    this.loading = true;
    const formValue = this.clusterForm.value;
    
    if (!this.isEditMode) {
      // New cluster mode: test connection with connection details
      const testData = {
        fe_host: formValue.fe_host,
        fe_http_port: formValue.fe_http_port,
        fe_query_port: formValue.fe_query_port,
        username: formValue.username,
        password: formValue.password,
        enable_ssl: formValue.enable_ssl || false,
        catalog: formValue.catalog || 'default_catalog',
      };

      this.clusterService.testConnection(testData).subscribe({
        next: (health) => this.handleHealthCheckResult(health),
        error: (error) => this.handleHealthCheckError(error),
      });
    } else {
      // Edit mode: check health of existing cluster
      this.clusterService.getHealth(this.clusterId).subscribe({
        next: (health) => this.handleHealthCheckResult(health),
        error: (error) => this.handleHealthCheckError(error),
      });
    }
  }

  private handleHealthCheckResult(health: any): void {
    if (health.status === 'healthy') {
      const details = health.checks.map((c: any) => c.name + ': ' + c.message).join('\n');
      this.toastrService.success('健康检查通过\n' + details, '连接成功');
    } else if (health.status === 'warning') {
      const details = health.checks.map((c: any) => c.name + ': ' + c.message).join('\n');
      this.toastrService.warning('健康检查发现问题\n' + details, '警告');
    } else {
      const details = health.checks.map((c: any) => c.name + ': ' + c.message).join('\n');
      this.toastrService.danger('健康检查失败\n' + details, '连接失败');
    }
    this.loading = false;
  }

  private handleHealthCheckError(error: any): void {
    this.toastrService.danger(
      ErrorHandler.extractErrorMessage(error),
      '错误',
    );
    this.loading = false;
  }

  private navigateToDashboardWithRefresh(): void {
    this.tabReuseService.markForRefresh('/pages/starrocks/dashboard');
    this.router.navigate(['/pages/starrocks/dashboard']);
  }
}


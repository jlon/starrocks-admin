import { Component, Input, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { NbDialogRef } from '@nebular/theme';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';

import {
  CreateOrganizationRequest,
  Organization,
  OrganizationService,
} from '../../../../@core/data/organization.service';
import { UserService, UserWithRoles } from '../../../../@core/data/user.service';

export type OrganizationFormMode = 'create' | 'edit';

export interface OrganizationFormDialogResult {
  mode: OrganizationFormMode;
  code?: string;
  name?: string;
  description?: string;
  admin_username?: string;
  admin_password?: string;
  admin_email?: string;
  admin_user_id?: number;
}

@Component({
  selector: 'ngx-organization-form-dialog',
  templateUrl: './organization-form-dialog.component.html',
  styleUrls: ['./organization-form-dialog.component.scss'],
})
export class OrganizationFormDialogComponent implements OnInit {
  @Input() mode: OrganizationFormMode = 'create';
  @Input() organization?: Organization;

  form: FormGroup;
  adminMode: 'create' | 'existing' = 'create';
  availableUsers$: Observable<UserWithRoles[]> | null = null;

  constructor(
    private dialogRef: NbDialogRef<OrganizationFormDialogComponent>,
    private fb: FormBuilder,
    private organizationService: OrganizationService,
    private userService: UserService,
  ) {
    this.form = this.fb.group({
      code: ['', [Validators.required, Validators.maxLength(50), Validators.pattern(/^[a-z0-9_]+$/)]],
      name: ['', [Validators.required, Validators.maxLength(100)]],
      description: ['', [Validators.maxLength(500)]],
      adminMode: ['create'],
      admin_username: ['', [Validators.maxLength(50)]],
      admin_password: ['', [Validators.minLength(6)]],
      admin_email: ['', [Validators.email, Validators.maxLength(100)]],
      admin_user_id: [null],
    });
  }

  ngOnInit(): void {
    if (this.mode === 'edit' && this.organization) {
      this.form.patchValue({
        code: this.organization.code,
        name: this.organization.name,
        description: this.organization.description || '',
      });
      this.form.get('code')?.disable();
      // Hide admin fields in edit mode
      this.form.get('adminMode')?.disable();
      this.form.get('admin_username')?.disable();
      this.form.get('admin_password')?.disable();
      this.form.get('admin_email')?.disable();
      this.form.get('admin_user_id')?.disable();
    } else {
      // In create mode, load available users for selection
      this.availableUsers$ = this.userService.listUsers().pipe(
        map((users) => users.filter((u) => !u.organization_id)), // Only show users without organization
      );
    }

    // Watch admin mode changes
    this.form.get('adminMode')?.valueChanges.subscribe((mode) => {
      this.adminMode = mode;
      this.updateAdminValidators();
    });

    this.updateAdminValidators();
  }

  private updateAdminValidators(): void {
    const usernameControl = this.form.get('admin_username');
    const passwordControl = this.form.get('admin_password');
    const emailControl = this.form.get('admin_email');
    const userIdControl = this.form.get('admin_user_id');

    if (this.adminMode === 'create') {
      usernameControl?.setValidators([Validators.required, Validators.maxLength(50)]);
      passwordControl?.setValidators([Validators.required, Validators.minLength(6)]);
      emailControl?.setValidators([Validators.email, Validators.maxLength(100)]);
      userIdControl?.clearValidators();
    } else {
      usernameControl?.clearValidators();
      passwordControl?.clearValidators();
      emailControl?.clearValidators();
      userIdControl?.setValidators([Validators.required]);
    }

    usernameControl?.updateValueAndValidity();
    passwordControl?.updateValueAndValidity();
    emailControl?.updateValueAndValidity();
    userIdControl?.updateValueAndValidity();
  }

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

    const formValue = this.form.getRawValue();

    if (this.mode === 'create') {
      // Validate admin information based on mode
      if (this.adminMode === 'create') {
        if (!formValue.admin_username || !formValue.admin_password) {
          this.form.get('admin_username')?.markAsTouched();
          this.form.get('admin_password')?.markAsTouched();
          return;
        }
      } else {
        if (!formValue.admin_user_id) {
          this.form.get('admin_user_id')?.markAsTouched();
          return;
        }
      }
    }

    const result: OrganizationFormDialogResult = {
      mode: this.mode,
      code: formValue.code,
      name: formValue.name,
      description: formValue.description || undefined,
    };

    if (this.mode === 'create') {
      if (this.adminMode === 'create') {
        result.admin_username = formValue.admin_username;
        result.admin_password = formValue.admin_password;
        result.admin_email = formValue.admin_email || undefined;
      } else {
        result.admin_user_id = formValue.admin_user_id;
      }
    }

    this.dialogRef.close(result);
  }

  cancel(): void {
    this.dialogRef.close();
  }
}


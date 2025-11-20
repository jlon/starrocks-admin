import { Component, Input, OnInit } from '@angular/core';
import { AbstractControl, FormBuilder, FormGroup, ValidationErrors, Validators } from '@angular/forms';
import { NbDialogRef } from '@nebular/theme';

import {
  CreateUserPayload,
  UpdateUserPayload,
  UserWithRoles,
} from '../../../../@core/data/user.service';
import { RoleWithPermissions } from '../../../../@core/data/role.service';
import { DiceBearService } from '../../../../@core/services/dicebear.service';
import { Organization } from '../../../../@core/data/organization.service';
import { AuthService } from '../../../../@core/data/auth.service';

export type UserFormMode = 'create' | 'edit';

export interface UserFormDialogResult {
  mode: UserFormMode;
  payload: CreateUserPayload | UpdateUserPayload;
}

@Component({
  selector: 'ngx-user-form-dialog',
  templateUrl: './user-form-dialog.component.html',
  styleUrls: ['./user-form-dialog.component.scss'],
})
export class UserFormDialogComponent implements OnInit {
  @Input() mode: UserFormMode = 'create';
  @Input() user?: UserWithRoles;
  @Input() roles: RoleWithPermissions[] = [];
  @Input() organizations: Organization[] = [];
  @Input() currentOrganization?: Organization;

  form: FormGroup;
  loading = false;
  isSuperAdmin = false;
  filteredRoles: RoleWithPermissions[] = [];

  constructor(
    private dialogRef: NbDialogRef<UserFormDialogComponent>,
    private fb: FormBuilder,
    private diceBearService: DiceBearService,
    private authService: AuthService,
  ) {
    this.form = this.fb.group({
      organizationId: [null],
      username: ['', [Validators.required, Validators.minLength(3), Validators.maxLength(50)]],
      email: ['', [Validators.required, Validators.email, Validators.maxLength(100)]],
      password: ['', [Validators.required, Validators.minLength(6)]],
      confirmPassword: ['', [Validators.required]],
      roleIds: [[], [Validators.required]],
    }, { validators: this.passwordMatchValidator.bind(this) });
  }

  ngOnInit(): void {
    // Determine if current user is super admin
    this.isSuperAdmin = this.authService.isSuperAdmin();
    
    // Set up organization field based on user role
    if (this.isSuperAdmin) {
      this.form.get('organizationId')?.setValidators([Validators.required]);
    } else {
      // Organization admin: auto-set organization
      if (this.currentOrganization) {
        this.form.get('organizationId')?.setValue(this.currentOrganization.id);
      }
      this.form.get('organizationId')?.disable();
    }
    this.form.get('organizationId')?.updateValueAndValidity();

    // Filter roles based on organization and user type
    this.updateFilteredRoles();

    if (this.mode === 'edit' && this.user) {
      // In edit mode, password is optional
      this.form.get('password')?.clearValidators();
      this.form.get('password')?.setValidators([
        (control: AbstractControl) => {
          const value = control.value;
          if (!value || value.trim() === '') {
            return null; // Empty password is allowed in edit mode
          }
          if (value.length < 6) {
            return { minlength: { requiredLength: 6, actualLength: value.length } };
          }
          return null;
        },
      ]);
      this.form.get('confirmPassword')?.clearValidators();
      this.form.get('confirmPassword')?.setValidators([
        (control: AbstractControl) => {
          const password = this.form.get('password')?.value;
          const confirmPassword = control.value;
          if (!password || password.trim() === '') {
            return null; // If password is empty, confirmation is not required
          }
          if (!confirmPassword || confirmPassword.trim() === '') {
            return { required: true };
          }
          if (confirmPassword.length < 6) {
            return { minlength: { requiredLength: 6, actualLength: confirmPassword.length } };
          }
          return null;
        },
      ]);

      this.form.patchValue({
        organizationId: this.user.organization_id,
        username: this.user.username,
        email: this.user.email || '',
        password: '',
        confirmPassword: '',
        roleIds: this.user.roles.map((role) => role.id),
      });
    } else {
      // In create mode, password is required
      this.form.get('password')?.setValidators([Validators.required, Validators.minLength(6)]);
      this.form.get('confirmPassword')?.setValidators([Validators.required]);
    }

    // Update validators after patching values
    this.form.get('password')?.updateValueAndValidity();
    this.form.get('confirmPassword')?.updateValueAndValidity();

    // Listen to organization changes to update filtered roles
    this.form.get('organizationId')?.valueChanges.subscribe(() => {
      this.updateFilteredRoles();
    });
  }

  private updateFilteredRoles(): void {
    const selectedOrgId = this.form.get('organizationId')?.value;
    
    if (this.isSuperAdmin) {
      // Super admin can see all roles
      this.filteredRoles = this.roles;
    } else {
      // Organization admin can only see roles in their organization
      // (or system roles that are not super_admin)
      this.filteredRoles = this.roles.filter(role => {
        // Exclude super_admin role for org admins
        if (role.code === 'super_admin') {
          return false;
        }
        // Include roles from current organization or system roles
        return !role.organization_id || role.organization_id === this.currentOrganization?.id;
      });
    }
  }

  passwordMatchValidator(control: AbstractControl): ValidationErrors | null {
    const password = control.get('password')?.value;
    const confirmPassword = control.get('confirmPassword')?.value;

    // In edit mode, if password is empty, no need to validate
    if (!password || password.trim() === '') {
      return null;
    }

    // If password is provided, confirmPassword must match
    if (password && confirmPassword && password !== confirmPassword) {
      return { passwordMismatch: true };
    }
    return null;
  }

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

    const formValue = this.form.getRawValue();

    // Additional validation for password confirmation
    if (formValue.password && formValue.password !== formValue.confirmPassword) {
      this.form.setErrors({ passwordMismatch: true });
      this.form.get('confirmPassword')?.setErrors({ passwordMismatch: true });
      this.form.markAllAsTouched();
      return;
    }

    const payload = this.buildPayload();
    this.dialogRef.close({
      mode: this.mode,
      payload,
    });
  }

  cancel(): void {
    this.dialogRef.close();
  }

  private buildPayload(): CreateUserPayload | UpdateUserPayload {
    const { organizationId, username, email, password, roleIds } = this.form.getRawValue();
    const trimmedUsername = username.trim();
    const trimmedEmail = email?.trim() || undefined;

    // Generate random avatar if not provided (using username as seed for consistency)
    // In create mode: always generate avatar
    // In edit mode: keep existing avatar if available, otherwise generate new one
    let avatar: string;
    if (this.mode === 'create') {
      avatar = this.diceBearService.generateAvatarForUser(trimmedUsername, 'avataaars');
    } else {
      // In edit mode, keep existing avatar if user has one, otherwise generate new one
      avatar = this.user?.avatar || this.diceBearService.generateAvatarForUser(trimmedUsername, 'avataaars');
    }

    if (this.mode === 'create') {
      const createPayload: CreateUserPayload = {
        username: trimmedUsername,
        password: password,
        email: trimmedEmail,
        role_ids: roleIds,
        avatar: avatar,
      };
      
      // Only super admin can specify organization_id
      if (this.isSuperAdmin && organizationId) {
        createPayload.organization_id = organizationId;
      }
      
      return createPayload;
    }

    const updatePayload: UpdateUserPayload = {
      username: trimmedUsername,
      email: trimmedEmail,
      role_ids: roleIds,
      avatar: avatar,
    };
    
    // Only super admin can change organization_id
    if (this.isSuperAdmin && organizationId) {
      updatePayload.organization_id = organizationId;
    }

    // Only include password if it's provided
    if (password && password.trim()) {
      updatePayload.password = password;
    }

    return updatePayload;
  }
}


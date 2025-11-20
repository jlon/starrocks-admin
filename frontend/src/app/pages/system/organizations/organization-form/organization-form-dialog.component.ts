import { Component, Input, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { NbDialogRef } from '@nebular/theme';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';

import { Organization } from '../../../../@core/data/organization.service';
import { UserService, UserWithRoles } from '../../../../@core/data/user.service';

export type OrganizationFormMode = 'create' | 'edit';

export interface OrganizationFormDialogResult {
  mode: OrganizationFormMode;
  code?: string;
  name?: string;
  description?: string;
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
  availableUsers$: Observable<UserWithRoles[]> | null = null;

  constructor(
    private dialogRef: NbDialogRef<OrganizationFormDialogComponent>,
    private fb: FormBuilder,
    private userService: UserService,
  ) {
    this.form = this.fb.group({
      code: ['', [Validators.required, Validators.maxLength(50), Validators.pattern(/^[a-z0-9_]+$/)]],
      name: ['', [Validators.required, Validators.maxLength(100)]],
      description: ['', [Validators.maxLength(500)]],
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
      this.loadOrganizationUsers(this.organization.id);
      this.form.get('admin_user_id')?.setValue(null);
    } else {
      this.form.get('admin_user_id')?.disable();
    }
  }

  private loadOrganizationUsers(orgId: number): void {
    this.availableUsers$ = this.userService.listUsers().pipe(
      map((users) => users.filter((u) => u.organization_id === orgId)),
    );
  }

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

    const formValue = this.form.getRawValue();

    const result: OrganizationFormDialogResult = {
      mode: this.mode,
      code: formValue.code,
      name: formValue.name,
      description: formValue.description || undefined,
    };

    if (this.mode === 'edit') {
      result.admin_user_id = formValue.admin_user_id || undefined;
    }

    this.dialogRef.close(result);
  }

  cancel(): void {
    this.dialogRef.close();
  }
}


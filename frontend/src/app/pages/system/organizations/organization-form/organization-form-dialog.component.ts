import { Component, Input, OnInit, OnDestroy } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { NbDialogRef } from '@nebular/theme';
import { Subject } from 'rxjs';

import { Organization } from '../../../../@core/data/organization.service';
import { UserWithRoles } from '../../../../@core/data/user.service';

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
export class OrganizationFormDialogComponent implements OnInit, OnDestroy {
  @Input() mode: OrganizationFormMode = 'create';
  @Input() organization?: Organization;
  @Input() availableUsers: UserWithRoles[] = [];

  form: FormGroup;
  
  private destroy$ = new Subject<void>();

  constructor(
    private dialogRef: NbDialogRef<OrganizationFormDialogComponent>,
    private fb: FormBuilder,
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
      
      // Set admin_user_id if present (similar to role form)
      if (this.organization.admin_user_id) {
        this.form.get('admin_user_id')?.setValue(this.organization.admin_user_id);
      }
    } else {
      this.form.get('admin_user_id')?.disable();
    }
  }
  
  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
  }
  
  trackByUserId(index: number, user: UserWithRoles): number {
    return user.id;
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


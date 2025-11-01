import { Component, Input, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { NbDialogRef } from '@nebular/theme';

import {
  CreateRolePayload,
  PermissionDto,
  RoleSummary,
  UpdateRolePayload,
} from '../../../../@core/data/role.service';

export type RoleFormMode = 'create' | 'edit';

export interface RoleFormDialogResult {
  mode: RoleFormMode;
  rolePayload: CreateRolePayload | UpdateRolePayload;
  permissionIds: number[];
}

interface MenuPermission {
  id: number;
  name: string;
  code: string;
}

@Component({
  selector: 'ngx-role-form-dialog',
  templateUrl: './role-form-dialog.component.html',
  styleUrls: ['./role-form-dialog.component.scss'],
})
export class RoleFormDialogComponent implements OnInit {
  @Input() mode: RoleFormMode = 'create';
  @Input() role?: RoleSummary;
  @Input() permissions: PermissionDto[] = [];

  form: FormGroup;
  menuPermissions: MenuPermission[] = [];

  // Maps for menu-API associations
  private menuToApis = new Map<number, PermissionDto[]>();
  private apiToMenus = new Map<number, number[]>();

  constructor(
    private dialogRef: NbDialogRef<RoleFormDialogComponent>,
    private fb: FormBuilder,
  ) {
    this.form = this.fb.group({
      code: ['', [Validators.required, Validators.maxLength(50)]],
      name: ['', [Validators.required, Validators.maxLength(50)]],
      description: ['', [Validators.maxLength(200)]],
      menuIds: [[], [Validators.required]],
    });
  }

  ngOnInit(): void {
    // Build menu-API associations
    this.buildApiAssociations();

    // Extract menu permissions and build menu list
    this.extractMenuPermissions();

    // Get initially selected menu IDs from permissions
    const initialMenuIds = this.permissions
      .filter((perm) => perm.type === 'menu' && perm.selected)
      .map((perm) => perm.id);

    // Sync APIs with initially selected menus
    if (initialMenuIds.length > 0) {
      this.syncApisWithSelectedMenus();
    }

    if (this.mode === 'edit' && this.role) {
      this.form.patchValue({
        code: this.role.code,
        name: this.role.name,
        description: this.role.description ?? '',
        menuIds: initialMenuIds,
      });
      this.form.get('code')?.disable();
    } else {
      this.form.patchValue({
        menuIds: initialMenuIds,
      });
    }
  }

  onMenuSelectionChange(selectedMenuIds: number[]): void {
    // Get previous selection to detect changes
    const previousMenuIds = this.form.get('menuIds')?.value || [];

    // Update the original permission object's selected state
    this.menuPermissions.forEach((menu) => {
      const wasSelected = previousMenuIds.includes(menu.id);
      const isSelected = selectedMenuIds.includes(menu.id);

      const permission = this.permissions.find((p) => p.id === menu.id);
      if (permission) {
        permission.selected = isSelected;
      }

      // Automatically authorize associated APIs when selection changes
      if (wasSelected !== isSelected) {
        this.applyApiSelectionForMenu(menu.id, isSelected);
      }
    });
  }

  submit(): void {
    if (this.form.invalid) {
      this.form.markAllAsTouched();
      return;
    }

    const selectedMenuIds = this.form.get('menuIds')?.value || [];
    if (!selectedMenuIds.length) {
      this.form.get('menuIds')?.markAsTouched();
      return;
    }

    // Get all permission IDs (selected menus + their associated APIs)
    const allSelectedPermissionIds = new Set<number>();

    // Add selected menu IDs
    selectedMenuIds.forEach((id: number) => allSelectedPermissionIds.add(id));

    // Add associated API IDs for selected menus
    selectedMenuIds.forEach((menuId: number) => {
      const relatedApis = this.menuToApis.get(menuId);
      if (relatedApis) {
        relatedApis.forEach((api) => allSelectedPermissionIds.add(api.id));
      }
    });

    const permissionIds = Array.from(allSelectedPermissionIds);

    const payload = this.buildPayload();
    this.dialogRef.close({
      mode: this.mode,
      rolePayload: payload,
      permissionIds,
    });
  }

  cancel(): void {
    this.dialogRef.close();
  }

  private extractMenuPermissions(): void {
    const menus = this.permissions.filter((perm) => perm.type === 'menu');

    this.menuPermissions = menus.map((menu) => ({
      id: menu.id,
      name: menu.name,
      code: menu.code,
    }));
  }

  private buildApiAssociations(): void {
    const menus = this.permissions.filter((perm) => perm.type === 'menu');
    const apis = this.permissions.filter((perm) => perm.type === 'api');

    if (!menus.length || !apis.length) {
      return;
    }

    // Extract menu paths from menu codes
    const menuPaths = menus
      .map((menu) => ({
        id: menu.id,
        path: this.extractPermissionPath(menu.code) || menu.code || '',
      }))
      .sort((a, b) => b.path.length - a.path.length);

    // Build associations between APIs and menus
    apis.forEach((api) => {
      const relatedMenuIds = new Set<number>();

      // Check parent_id relationship
      if (api.parent_id) {
        const parentMenu = menus.find((m) => m.id === api.parent_id);
        if (parentMenu) {
          relatedMenuIds.add(api.parent_id);
        }
      }

      // Check code/resource matching
      const apiPath = this.extractPermissionPath(api.code) || api.resource || '';
      if (apiPath) {
        for (const menuPath of menuPaths) {
          if (!menuPath.path) {
            continue;
          }
          // Match if API path starts with menu path or equals menu path
          if (apiPath === menuPath.path || apiPath.startsWith(`${menuPath.path}:`)) {
            relatedMenuIds.add(menuPath.id);
            break;
          }
        }
      }

      // Fallback: check resource matching
      if (!relatedMenuIds.size && api.resource) {
        menus.forEach((menu) => {
          const menuPath = this.extractPermissionPath(menu.code) || '';
          if (!menuPath) {
            return;
          }
          if (
            menuPath === api.resource ||
            api.resource.startsWith(menuPath) ||
            menuPath.startsWith(api.resource)
          ) {
            relatedMenuIds.add(menu.id);
          }
        });
      }

      // Store associations
      if (relatedMenuIds.size > 0) {
        const menuIdArray = Array.from(relatedMenuIds);
        this.apiToMenus.set(api.id, menuIdArray);

        menuIdArray.forEach((menuId) => {
          if (!this.menuToApis.has(menuId)) {
            this.menuToApis.set(menuId, []);
          }
          this.menuToApis.get(menuId)!.push(api);
        });
      }
    });
  }

  private applyApiSelectionForMenu(menuId: number, selected: boolean): void {
    const relatedApis = this.menuToApis.get(menuId);
    if (!relatedApis || !relatedApis.length) {
      return;
    }

    const selectedMenuIds = this.form.get('menuIds')?.value || [];

    relatedApis.forEach((api) => {
      if (selected) {
        // Menu selected: automatically select associated API
        api.selected = true;
      } else {
        // Menu deselected: check if API should still be selected
        // API should remain selected if any other related menu is still selected
        const relatedMenuIds = this.apiToMenus.get(api.id) || [];
        const shouldRemainSelected = relatedMenuIds.some((id) => selectedMenuIds.includes(id));

        if (!shouldRemainSelected) {
          api.selected = false;
        }
      }
    });
  }

  private syncApisWithSelectedMenus(): void {
    // When initializing, sync all APIs with their related selected menus
    const selectedMenuIds = this.permissions
      .filter((perm) => perm.type === 'menu' && perm.selected)
      .map((perm) => perm.id);

    selectedMenuIds.forEach((menuId) => {
      this.applyApiSelectionForMenu(menuId, true);
    });
  }

  private extractPermissionPath(code?: string): string | null {
    if (!code) {
      return null;
    }
    // Extract path from code like "menu:dashboard" -> "dashboard"
    const parts = code.split(':');
    return parts.length > 1 ? parts.slice(1).join(':') : code;
  }

  private buildPayload(): CreateRolePayload | UpdateRolePayload {
    const { code, name, description } = this.form.getRawValue();
    const normalizedName = (name ?? '').trim();
    const normalizedDescription = description?.trim() || undefined;

    if (this.mode === 'create') {
      return {
        code: (code ?? '').trim(),
        name: normalizedName,
        description: normalizedDescription,
      };
    }

    return {
      name: normalizedName,
      description: normalizedDescription,
    };
  }
}

import { Component, Input, OnInit } from '@angular/core';
import { FormBuilder, FormGroup, Validators } from '@angular/forms';
import { NbDialogRef } from '@nebular/theme';

import {
  CreateRolePayload,
  PermissionDto,
  RoleSummary,
  UpdateRolePayload,
} from '../../../../@core/data/role.service';
import { Organization } from '../../../../@core/data/organization.service';
import { PermissionService } from '../../../../@core/data/permission.service';
import { AuthService } from '../../../../@core/data/auth.service';

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

// Tree node structure for hierarchical permission display (inspired by query-execution tree)
interface PermissionTreeNode {
  id: number;
  name: string;
  code: string;
  level: number;
  icon: string;
  parentId?: number;
  children: PermissionTreeNode[];
  checked: boolean;
  indeterminate: boolean;
  expanded: boolean; // Control collapse/expand state
  permission: PermissionDto; // Reference to original permission
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
  @Input() organizations: Organization[] = [];
  @Input() currentOrganization?: Organization;

  form: FormGroup;
  menuPermissions: MenuPermission[] = [];
  isSuperAdmin = false;

  // Simple tree structure (root nodes only, inspired by query-execution)
  permissionTree: PermissionTreeNode[] = [];
  
  // Node maps for quick lookup
  private nodeMap = new Map<number, PermissionTreeNode>();
  private parentMap = new Map<number, PermissionTreeNode>();

  // Maps for menu-API associations
  private menuToApis = new Map<number, PermissionDto[]>();
  private apiToMenus = new Map<number, number[]>();

  constructor(
    private dialogRef: NbDialogRef<RoleFormDialogComponent>,
    private fb: FormBuilder,
    private permissionService: PermissionService,
    private authService: AuthService,
  ) {
    this.form = this.fb.group({
      code: ['', [Validators.required, Validators.maxLength(50)]],
      name: ['', [Validators.required, Validators.maxLength(50)]],
      description: ['', [Validators.maxLength(200)]],
      organizationId: [null],
      menuIds: [[], [Validators.required]],
    });
  }

  ngOnInit(): void {
    // Determine if current user is super admin
    this.isSuperAdmin = this.authService.isSuperAdmin();

    const orgControl = this.form.get('organizationId');
    if (this.isSuperAdmin) {
      orgControl?.setValidators([Validators.required]);
    } else if (this.currentOrganization) {
      orgControl?.setValue(this.currentOrganization.id);
      orgControl?.disable();
    }
    if (this.mode === 'edit' && this.role?.organization_id) {
      orgControl?.setValue(this.role.organization_id);
      orgControl?.disable();
    }
    orgControl?.updateValueAndValidity();

    // Build menu-API associations
    this.buildApiAssociations();

    // Build permission tree structure
    this.buildPermissionTree();

    // Extract menu permissions (for backwards compatibility)
    this.extractMenuPermissions();

    // Sync APIs and reactive form with existing selections
    this.syncApisWithSelectedMenus();
    this.updateMenuSelectionControl({ triggerApiSync: false });

    if (this.mode === 'edit' && this.role) {
      this.form.patchValue({
        code: this.role.code,
        name: this.role.name,
        description: this.role.description ?? '',
      });
      this.form.get('code')?.disable();
    }
  }

  private applyMenuSelectionChanges(selectedMenuIds: number[]): void {
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
      .map((menu) => {
        const path = this.extractPermissionPath(menu.code) || menu.code || '';
        const segments = path ? path.split(':') : [];
        return {
          id: menu.id,
          path,
          segments,
        };
      })
      .sort((a, b) => b.path.length - a.path.length);

    // Build associations between APIs and menus
    apis.forEach((api) => {
      const relatedMenuIds = new Set<number>();

      if (api.parent_id) {
        const parentMenu = menus.find((m) => m.id === api.parent_id);
        if (parentMenu) {
          relatedMenuIds.add(api.parent_id);
        }
      }

      const apiPath = this.extractPermissionPath(api.code) || api.resource || '';
      const apiSegments = apiPath ? apiPath.split(':') : [];
      const apiFirst = apiSegments.length ? apiSegments[0] : undefined;
      const apiLast = apiSegments.length ? apiSegments[apiSegments.length - 1] : undefined;

      let bestMatchId: number | null = null;
      let bestScore = -1;

      if (apiPath) {
        for (const menuPath of menuPaths) {
          if (!menuPath.path) {
            continue;
          }

          const menuSegments = menuPath.segments;
          const menuFirst = menuSegments.length ? menuSegments[0] : undefined;
          const menuLast = menuSegments.length ? menuSegments[menuSegments.length - 1] : undefined;

          let score = 0;

          if (apiPath === menuPath.path || apiPath.startsWith(`${menuPath.path}:`)) {
            score = 100 + menuPath.path.length;
          } else {
            if (menuSegments.length > 1) {
              const matchesAll = menuSegments.every((segment) => apiSegments.includes(segment));
              if (matchesAll) {
                score = Math.max(score, 80 + menuPath.path.length);
              }
            }

            if (menuFirst && apiFirst && menuFirst === apiFirst) {
              score = Math.max(score, 70 + menuFirst.length);
            }

            if (menuLast && apiLast && menuLast === apiLast) {
              score = Math.max(score, 60 + menuLast.length);
            }

            if (menuLast && apiSegments.includes(menuLast)) {
              score = Math.max(score, 50 + menuLast.length);
            }
          }

          if (score > bestScore) {
            bestScore = score;
            bestMatchId = menuPath.id;
          }
        }
      }

      if (bestMatchId !== null && bestScore > 0) {
        relatedMenuIds.add(bestMatchId);
      }

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

  private updateMenuSelectionControl(options: { triggerApiSync?: boolean } = {}): void {
    const { triggerApiSync = true } = options;
    const selectedMenuIds = this.getSelectedMenuIds();

    if (triggerApiSync) {
      this.applyMenuSelectionChanges(selectedMenuIds);
    }

    const control = this.form.get('menuIds');
    control?.setValue(selectedMenuIds, { emitEvent: false });
  }

  private getSelectedMenuIds(): number[] {
    return this.permissions
      .filter((perm) => perm.type === 'menu' && perm.selected)
      .map((perm) => perm.id);
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
    const { code, name, description, organizationId } = this.form.getRawValue();
    const normalizedName = (name ?? '').trim();
    const normalizedDescription = description?.trim() || undefined;

    if (this.mode === 'create') {
      if (this.isSuperAdmin) {
        return {
          code: (code ?? '').trim(),
          name: normalizedName,
          description: normalizedDescription,
          organization_id: organizationId || undefined,
        };
      } else {
        return {
          code: (code ?? '').trim(),
          name: normalizedName,
          description: normalizedDescription,
        };
      }
    }

    return {
      name: normalizedName,
      description: normalizedDescription,
    };
  }

  // ============================================
  // Tree Structure Methods
  // ============================================

  /**
   * Build simple hierarchical permission tree (inspired by query-execution tree)
   * No grouping, just pure hierarchy
   */
  private buildPermissionTree(): void {
    const menuPermissions = this.permissions.filter((perm) => perm.type === 'menu');

    // Step 1: Create tree nodes and build node map
    menuPermissions.forEach((perm) => {
      const parts = perm.code.split(':');
      const level = parts.length - 1; // menu:nodes = 1, menu:nodes:backends = 2

      const node: PermissionTreeNode = {
        id: perm.id,
        name: perm.name,
        code: perm.code,
        level,
        icon: this.getPermissionIcon(perm.code),
        checked: perm.selected || false,
        indeterminate: false,
        expanded: false, // Default collapsed
        children: [],
        permission: perm,
      };

      this.nodeMap.set(perm.id, node);
    });

    // Step 2: Build parent-child relationships and collect root nodes
    const rootNodes: PermissionTreeNode[] = [];
    
    menuPermissions.forEach((perm) => {
      const node = this.nodeMap.get(perm.id);
      if (!node) return;

      // Find parent by matching code prefix
      const parentNode = this.findParentNodeByCode(perm.code);
      if (parentNode) {
        node.parentId = parentNode.id;
        parentNode.children.push(node);
        this.parentMap.set(node.id, parentNode);
      } else {
        // No parent: this is a root node
        rootNodes.push(node);
      }
    });

    // Step 3: Update indeterminate states based on initial selection
    this.nodeMap.forEach((node) => {
      if (node.children.length > 0) {
        this.updateNodeState(node);
      }
    });

    // Step 4: Set root nodes (sorted by name)
    this.permissionTree = rootNodes.sort((a, b) => a.name.localeCompare(b.name));
  }

  /**
   * Get icon for permission based on code
   */
  private getPermissionIcon(code: string): string {
    if (code.includes('dashboard')) return 'home-outline';
    if (code.includes('overview')) return 'bar-chart-outline';
    if (code.includes('nodes')) return 'cube-outline';
    if (code.includes('frontends') || code.includes('backends')) return 'hard-drive-outline';
    if (code.includes('queries') || code.includes('execution') || code.includes('profiles') || code.includes('audit')) return 'search-outline';
    if (code.includes('materialized-views')) return 'layers-outline';
    if (code.includes('sessions')) return 'people-outline';
    if (code.includes('variables')) return 'code-outline';
    if (code.includes('system:users')) return 'person-outline';
    if (code.includes('system:roles')) return 'shield-outline';
    if (code.includes('system:organizations')) return 'briefcase-outline';
    if (code.includes('system')) return 'settings-outline';
    if (code.includes('users')) return 'person-outline';
    if (code.includes('roles')) return 'shield-outline';
    if (code.includes('organizations')) return 'briefcase-outline';
    return 'folder-outline';
  }

  /**
   * Find parent node by matching code prefix
   */
  private findParentNodeByCode(code: string): PermissionTreeNode | null {
    const parts = code.split(':');
    if (parts.length <= 2) return null; // Top-level node, no parent

    // Try to find parent by removing last part
    // e.g., menu:nodes:backends -> menu:nodes
    const parentCode = parts.slice(0, parts.length - 1).join(':');

    for (const node of this.nodeMap.values()) {
      if (node.code === parentCode) {
        return node;
      }
    }

    return null;
  }


  /**
   * Handle checkbox change for a tree node
   */
  onNodeCheckChange(node: PermissionTreeNode, checked: boolean): void {
    node.checked = checked;
    node.indeterminate = false;

    // Update children recursively
    this.setChildrenChecked(node, checked);

    // Update parent states
    if (node.parentId) {
      const parent = this.parentMap.get(node.id);
      if (parent) {
        this.updateNodeState(parent);
      }
    }

    // Sync to original permissions array
    this.syncNodeToPermission(node);

    // Reflect menu selection in reactive form
    this.updateMenuSelectionControl();
  }

  /**
   * Toggle node expand/collapse state
   */
  toggleNode(node: PermissionTreeNode, event?: Event): void {
    if (event) {
      event.stopPropagation();
      event.preventDefault();
    }
    node.expanded = !node.expanded;
  }

  /**
   * Check if node is expandable (has children)
   */
  isNodeExpandable(node: PermissionTreeNode): boolean {
    return node.children && node.children.length > 0;
  }

  /**
   * Get node indent based on level (for styling)
   */
  getNodeIndent(node: PermissionTreeNode): number {
    // level 1: 12px, level 2: 32px, level 3: 52px
    return 12 + (node.level - 1) * 20;
  }

  /**
   * Track by function for ngFor optimization
   */
  trackNodeById(index: number, node: PermissionTreeNode): number {
    return node.id;
  }

  /**
   * Set all children to checked/unchecked recursively
   */
  private setChildrenChecked(node: PermissionTreeNode, checked: boolean): void {
    node.children.forEach((child) => {
      child.checked = checked;
      child.indeterminate = false;
      this.syncNodeToPermission(child);
      this.setChildrenChecked(child, checked);
    });
  }

  /**
   * Update node's checked and indeterminate state based on children
   */
  private updateNodeState(node: PermissionTreeNode): void {
    if (node.children.length === 0) return;

    const checkedCount = node.children.filter((c) => c.checked).length;
    const indeterminateCount = node.children.filter((c) => c.indeterminate).length;

    if (checkedCount === node.children.length) {
      // All children checked
      node.checked = true;
      node.indeterminate = false;
    } else if (checkedCount > 0 || indeterminateCount > 0) {
      // Some children checked or indeterminate
      node.checked = false;
      node.indeterminate = true;
    } else {
      // No children checked
      node.checked = false;
      node.indeterminate = false;
    }

    this.syncNodeToPermission(node);

    // Recursively update parent
    if (node.parentId) {
      const parent = this.parentMap.get(node.id);
      if (parent) {
        this.updateNodeState(parent);
      }
    }
  }

  /**
   * Sync tree node state to original permission object
   */
  private syncNodeToPermission(node: PermissionTreeNode): void {
    node.permission.selected = node.checked;
  }
}

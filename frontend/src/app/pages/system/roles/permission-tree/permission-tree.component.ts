import { Component, EventEmitter, Input, OnChanges, Output } from '@angular/core';

export interface Permission {
  id: number;
  code: string;
  name: string;
  type: 'menu' | 'api';
  resource?: string;
  action?: string;
  description?: string;
  parent_id?: number;
  selected?: boolean;
}

export interface PermissionTreeSelection {
  permissions: Permission[];
}

interface PermissionTreeNode {
  key: string;
  name: string;
  selectable: boolean;
  permission?: Permission;
  selected: boolean;
  indeterminate?: boolean;
  children?: PermissionTreeNode[];
  parentKey?: string | null;
  depth: number;
}

@Component({
  selector: 'ngx-permission-tree',
  templateUrl: './permission-tree.component.html',
  styleUrls: ['./permission-tree.component.scss'],
})
export class PermissionTreeComponent implements OnChanges {
  @Input() permissions: Permission[] = [];
  @Input() readonly = false;
  @Output() selectionChange = new EventEmitter<PermissionTreeSelection>();

  private treeData: PermissionTreeNode[] = [];
  private nodeIndex = new Map<string, PermissionTreeNode>();
  private menuPermissionIndex = new Map<number, Permission>();
  private menuToApis = new Map<number, Permission[]>();
  private apiToMenus = new Map<number, number[]>();
  private menuOrder = new Map<number, number>();

  get nodes(): PermissionTreeNode[] {
    return this.treeData;
  }

  ngOnChanges(): void {
    this.buildTree();
  }

  onToggle(key: string, checked: boolean): void {
    const node = this.nodeIndex.get(key);
    if (!node || !node.selectable) {
      return;
    }

    this.setNodeSelection(node, checked);
    this.updateParentState(node.parentKey);
    this.emitSelection();
  }

  private buildTree(): void {
    this.nodeIndex.clear();
    this.menuPermissionIndex.clear();
    this.menuToApis.clear();
    this.apiToMenus.clear();
    this.menuOrder.clear();

    const menus = this.permissions.filter((permission) => permission.type === 'menu');
    const apis = this.permissions.filter((permission) => permission.type === 'api');

    menus.forEach((permission, index) => {
      this.menuPermissionIndex.set(permission.id, permission);
      this.menuOrder.set(permission.id, index);
    });
    const roots = this.buildMenuTreeNodes(menus);
    this.buildApiAssociations(menus, apis);

    if (!roots.length) {
      this.treeData = [this.createPlaceholderNode('暂无菜单权限', null)];
    } else {
      this.treeData = roots;
    }

    this.treeData.forEach((node) => this.setDepth(node, 0));
    this.treeData.forEach((node) => this.registerNode(node));

    this.syncApisWithSelectedMenus();
    this.emitSelection();
  }

  private buildMenuTreeNodes(menus: Permission[]): PermissionTreeNode[] {
    if (!menus.length) {
      return [];
    }

    const nodeMap = new Map<number, PermissionTreeNode>();

    menus.forEach((permission) => {
      const node = this.createMenuNode(permission);
      nodeMap.set(permission.id, node);
    });

    const roots: PermissionTreeNode[] = [];

    menus.forEach((permission) => {
      const node = nodeMap.get(permission.id)!;
      const parentId = permission.parent_id;
      if (parentId && nodeMap.has(parentId)) {
        const parentNode = nodeMap.get(parentId)!;
        parentNode.children = parentNode.children || [];
        parentNode.children.push(node);
        node.parentKey = parentNode.key;
      } else {
        node.parentKey = null;
        roots.push(node);
      }
    });

    const sortByOrder = (a: PermissionTreeNode, b: PermissionTreeNode): number => {
      const aId = a.permission?.id ?? 0;
      const bId = b.permission?.id ?? 0;
      return (this.menuOrder.get(aId) ?? 0) - (this.menuOrder.get(bId) ?? 0);
    };

    const sortChildren = (nodes: PermissionTreeNode[]) => {
      nodes.sort(sortByOrder);
      nodes.forEach((node) => {
        if (node.children?.length) {
          sortChildren(node.children);
        }
      });
    };

    sortChildren(roots);
    return roots;
  }

  private createPlaceholderNode(label: string, parentKey: string | null): PermissionTreeNode {
    return {
      key: `placeholder-${parentKey}-${label}`,
      name: label,
      selectable: false,
      selected: false,
      parentKey,
      depth: 0,
    };
  }

  private createMenuNode(permission: Permission): PermissionTreeNode {
    return {
      key: `perm-${permission.id}`,
      name: permission.name,
      selectable: true,
      permission,
      selected: !!permission.selected,
      parentKey: null,
      depth: 0,
    };
  }

  private registerNode(node: PermissionTreeNode): void {
    this.nodeIndex.set(node.key, node);
    node.children?.forEach((child) => {
      child.parentKey = node.key;
      this.registerNode(child);
    });
    this.updateCategorySelection(node);
  }

  private setDepth(node: PermissionTreeNode, depth: number): void {
    node.depth = depth;
    node.children?.forEach((child) => this.setDepth(child, depth + 1));
  }

  private setNodeSelection(node: PermissionTreeNode, selected: boolean): void {
    if (!node.selectable) {
      return;
    }

    node.selected = selected;
    node.indeterminate = false;

    if (node.permission) {
      node.permission.selected = selected;
      if (node.permission.type === 'menu') {
        this.applyApiSelectionForMenu(node.permission.id, selected);
      }
    }

    node.children?.forEach((child) => this.setNodeSelection(child, selected));
  }

  private updateParentState(parentKey: string | null | undefined): void {
    if (!parentKey) {
      return;
    }

    const parent = this.nodeIndex.get(parentKey);
    if (!parent) {
      return;
    }

    this.updateCategorySelection(parent);
    this.updateParentState(parent.parentKey);
  }

  private updateCategorySelection(node: PermissionTreeNode): void {
    if (!node.children || !node.children.length) {
      return;
    }

    const selectableChildren = node.children.filter((child) => child.selectable);
    if (!selectableChildren.length) {
      node.selected = false;
      node.indeterminate = false;
      return;
    }

    const allSelected = selectableChildren.every((child) => child.selected && !child.indeterminate);
    const noneSelected = selectableChildren.every((child) => !child.selected && !child.indeterminate);

    node.selected = allSelected;
    node.indeterminate = !(allSelected || noneSelected);

    if (node.permission) {
      node.permission.selected = node.selected;
    }
  }

  private emitSelection(): void {
    this.selectionChange.emit({ permissions: this.permissions });
  }

  private buildApiAssociations(menus: Permission[], apis: Permission[]): void {
    if (!menus.length || !apis.length) {
      return;
    }

    const menuPaths = menus
      .map((menu) => ({ id: menu.id, path: this.extractPermissionPath(menu.code) || '' }))
      .sort((a, b) => b.path.length - a.path.length);

    apis.forEach((api) => {
      const relatedMenuIds = new Set<number>();

      if (api.parent_id && this.menuPermissionIndex.has(api.parent_id)) {
        relatedMenuIds.add(api.parent_id);
      }

      const apiPath = this.extractPermissionPath(api.code) || api.resource || '';
      if (apiPath) {
        for (const menuPath of menuPaths) {
          if (!menuPath.path) {
            continue;
          }
          if (apiPath === menuPath.path || apiPath.startsWith(`${menuPath.path}:`)) {
            relatedMenuIds.add(menuPath.id);
            break;
          }
        }
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

      if (!relatedMenuIds.size) {
        return;
      }

      const ids = Array.from(relatedMenuIds);
      this.apiToMenus.set(api.id, ids);
      ids.forEach((menuId) => {
        if (!this.menuToApis.has(menuId)) {
          this.menuToApis.set(menuId, []);
        }
        this.menuToApis.get(menuId)!.push(api);
      });
    });
  }

  private applyApiSelectionForMenu(menuId: number, selected: boolean): void {
    const relatedApis = this.menuToApis.get(menuId);
    if (!relatedApis || !relatedApis.length) {
      return;
    }

    relatedApis.forEach((api) => {
      if (selected) {
        api.selected = true;
        return;
      }

      const menuIds = this.apiToMenus.get(api.id) || [];
      const stillSelected = menuIds.some((id) => this.menuPermissionIndex.get(id)?.selected);
      api.selected = stillSelected;
    });
  }

  private syncApisWithSelectedMenus(): void {
    this.menuPermissionIndex.forEach((permission, menuId) => {
      if (permission.selected) {
        this.applyApiSelectionForMenu(menuId, true);
      }
    });
  }

  private extractPermissionPath(code?: string): string | null {
    if (!code) {
      return null;
    }
    const [, ...rest] = code.split(':');
    return rest.length ? rest.join(':') : code;
  }
}

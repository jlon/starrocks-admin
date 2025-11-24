import { Injectable } from '@angular/core';
import { NbMenuItem } from '@nebular/theme';
import { PermissionService } from '../data/permission.service';

@Injectable({
  providedIn: 'root',
})
export class MenuFilterService {
  constructor(private permissionService: PermissionService) {}

  /**
   * Filter menu items based on user permissions
   * Recursively filters menu items and their children
   */
  filterMenuItems(items: NbMenuItem[]): NbMenuItem[] {
    return items
      .map((item) => {
        // Check if item has permission in data attribute
        const permission = (item as any).data?.permission;
        if (permission) {
          // Extract menu code from permission string (e.g., 'menu:dashboard' -> 'dashboard')
          const menuCode = this.extractMenuCodeFromPermission(permission);
          if (!this.permissionService.hasMenuPermission(menuCode)) {
            return null; // No permission, filter out
          }
        } else if (item.link) {
          // Try to extract menu code from link if no explicit permission
          const menuCode = this.extractMenuCodeFromLink(item.link);
          if (menuCode && !this.permissionService.hasMenuPermission(menuCode)) {
            return null; // No permission, filter out
          }
        }

        // Recursively filter children
        if (item.children && item.children.length > 0) {
          item.children = this.filterMenuItems(item.children);
          // If all children are filtered out, filter out parent too
          if (item.children.length === 0 && item.link) {
            return null;
          }
        }

        return item;
      })
      .filter((item) => item !== null) as NbMenuItem[];
  }

  /**
   * Extract menu code from permission string
   * Examples: 'menu:dashboard' -> 'dashboard'
   *           'menu:system:users' -> 'system:users'
   *           'menu:system:organizations' -> 'system:organizations'
   */
  private extractMenuCodeFromPermission(permission: string): string {
    const parts = permission.split(':');
    if (parts.length >= 2 && parts[0] === 'menu') {
      // Return everything after 'menu:' to support hierarchical permissions
      return parts.slice(1).join(':');
    }
    return permission;
  }

  /**
   * Extract menu code from link
   * Examples: '/pages/starrocks/dashboard' -> 'dashboard'
   *           '/pages/starrocks/queries/execution' -> 'queries'
   */
  private extractMenuCodeFromLink(link: string): string | null {
    if (!link) {
      return null;
    }

    // Remove leading slash and split by '/'
    const parts = link.replace(/^\//, '').split('/');
    
    // Try to find starrocks segment and get next segment
    const starrocksIndex = parts.indexOf('starrocks');
    if (starrocksIndex >= 0 && starrocksIndex < parts.length - 1) {
      const menuSegment = parts[starrocksIndex + 1];
      
      // Map common segments to menu codes
      const segmentMap: { [key: string]: string } = {
        dashboard: 'dashboard',
        overview: 'overview',
        frontends: 'nodes',
        backends: 'nodes',
        queries: 'queries',
        'materialized-views': 'materialized-views',
        system: 'system',
        sessions: 'sessions',
        variables: 'variables',
      };

      return segmentMap[menuSegment] || menuSegment;
    }

    return null;
  }
}


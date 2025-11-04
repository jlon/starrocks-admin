import { Injectable } from '@angular/core';
import { Router, CanActivate, ActivatedRouteSnapshot, RouterStateSnapshot } from '@angular/router';
import { PermissionService } from '../data/permission.service';

@Injectable({
  providedIn: 'root',
})
export class PermissionGuard implements CanActivate {
  constructor(
    private permissionService: PermissionService,
    private router: Router,
  ) {}

  canActivate(
    route: ActivatedRouteSnapshot,
    state: RouterStateSnapshot,
  ): boolean {
    const requiredPermission = route.data['permission'] as string;
    
    // If no permission required, allow access
    if (!requiredPermission) {
      return true;
    }
 
    // First try exact match with full permission code
    if (this.permissionService.hasPermission(requiredPermission)) {
      return true;
    }

    // Fallback: split into base code + action (supports legacy usage like menu:users:view)
    const parts = requiredPermission.split(':');
    if (parts.length >= 3) {
      const action = parts.pop() as string;
      const baseCode = parts.join(':');
      if (this.permissionService.hasPermission(baseCode, action)) {
        return true;
      }
    } else if (parts.length < 2) {
      console.warn('Invalid permission format:', requiredPermission);
      return true; // Allow access if format is invalid (graceful degradation)
    }
 
    console.warn('Permission denied:', requiredPermission, 'at', state.url);
    
    // DO NOT redirect - let the component handle "no permission" display
    // This avoids infinite redirect loops
    return false;
  }
}


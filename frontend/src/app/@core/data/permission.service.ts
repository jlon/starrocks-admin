import { Injectable } from '@angular/core';
import { BehaviorSubject, Observable, of } from 'rxjs';
import { tap, catchError, map } from 'rxjs/operators';
import { ApiService } from './api.service';

export interface Permission {
  id: number;
  code: string;
  name: string;
  type: 'menu' | 'api';
  resource?: string;
  action?: string;
  parent_id?: number;
  description?: string;
}

@Injectable({
  providedIn: 'root',
})
export class PermissionService {
  private permissionsSubject = new BehaviorSubject<Permission[]>([]);
  public permissions$: Observable<Permission[]>;
  private permissionsKey = 'user_permissions';
  private initialized = false;

  constructor(private api: ApiService) {
    this.permissions$ = this.permissionsSubject.asObservable();
    // Try to load permissions from localStorage on init
    this.loadFromStorage();
  }

  /**
   * Initialize permissions from API
   * Should be called after user login
   */
  initPermissions(): Observable<Permission[]> {
    if (this.initialized) {
      return this.permissions$;
    }

    return this.api.get<Permission[]>('/auth/permissions').pipe(
      tap((permissions) => {
        this.permissionsSubject.next(permissions);
        this.saveToStorage(permissions);
        this.initialized = true;
      }),
      catchError((error) => {
        console.error('Failed to load permissions:', error);
        // Return empty array on error
        const emptyPermissions: Permission[] = [];
        this.permissionsSubject.next(emptyPermissions);
        return of(emptyPermissions);
      }),
    );
  }

  /**
   * Clear permissions (call on logout)
   */
  clearPermissions(): void {
    this.permissionsSubject.next([]);
    localStorage.removeItem(this.permissionsKey);
    this.initialized = false;
  }

  /**
   * Check if user has a specific permission
   */
  hasPermission(code: string, action?: string): boolean {
    const permissions = this.permissionsSubject.value;

    if (action) {
      const combinedCode = `${code}:${action}`;
      return permissions.some(
        (p) =>
          p.code === combinedCode ||
          (p.code === code && (!p.action || p.action === action))
      );
    }

    return permissions.some((p) => p.code === code);
  }

  /**
   * Check menu permission
   */
  hasMenuPermission(menuCode: string): boolean {
    return this.hasPermission(`menu:${menuCode}`, 'view');
  }

  /**
   * Check API permission
   */
  hasApiPermission(resource: string, action: string): boolean {
    return this.hasPermission(`api:${resource}`, action);
  }

  /**
   * Get all permissions as observable
   */
  getPermissions(): Observable<Permission[]> {
    return this.permissions$;
  }

  /**
   * Get current permissions synchronously
   */
  getCurrentPermissions(): Permission[] {
    return this.permissionsSubject.value;
  }

  /**
   * Refresh permissions from API
   */
  refreshPermissions(): Observable<Permission[]> {
    this.initialized = false;
    return this.initPermissions();
  }

  /**
   * Load permissions from localStorage
   */
  private loadFromStorage(): void {
    try {
      const stored = localStorage.getItem(this.permissionsKey);
      if (stored) {
        const permissions: Permission[] = JSON.parse(stored);
        this.permissionsSubject.next(permissions);
      }
    } catch (error) {
      console.error('Failed to load permissions from storage:', error);
    }
  }

  /**
   * Save permissions to localStorage
   */
  private saveToStorage(permissions: Permission[]): void {
    try {
      localStorage.setItem(this.permissionsKey, JSON.stringify(permissions));
    } catch (error) {
      console.error('Failed to save permissions to storage:', error);
    }
  }
}


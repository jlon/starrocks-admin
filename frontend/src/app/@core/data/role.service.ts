import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import { ApiService } from './api.service';

export interface RoleSummary {
  id: number;
  code: string;
  name: string;
  description?: string;
  is_system: boolean;
  organization_id?: number;
  created_at: string;
}

export interface CreateRolePayload {
  code: string;
  name: string;
  description?: string;
  organization_id?: number;
}

export interface UpdateRolePayload {
  name?: string;
  description?: string;
}

export interface PermissionDto {
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

export interface RoleWithPermissions extends RoleSummary {
  permissions: PermissionDto[];
}

interface RolePermissionsResponse {
  role: RoleSummary;
  permissions?: PermissionDto[];
}

@Injectable({
  providedIn: 'root',
})
export class RoleService {
  constructor(private api: ApiService) {}

  listRoles(): Observable<RoleSummary[]> {
    return this.api.get<RoleSummary[]>('/roles');
  }

  createRole(payload: CreateRolePayload): Observable<RoleSummary> {
    return this.api.post<RoleSummary>('/roles', payload);
  }

  updateRole(roleId: number, payload: UpdateRolePayload): Observable<RoleSummary> {
    return this.api.put<RoleSummary>(`/roles/${roleId}`, payload);
  }

  deleteRole(roleId: number): Observable<void> {
    return this.api.delete<void>(`/roles/${roleId}`);
  }

  listPermissions(): Observable<PermissionDto[]> {
    return this.api.get<PermissionDto[]>('/permissions');
  }

  getRolePermissions(roleId: number): Observable<PermissionDto[]> {
    return this.api
      .get<RolePermissionsResponse>(`/roles/${roleId}/permissions`)
      .pipe(
        map((response) => {
          const permissions = response?.permissions;

          if (!Array.isArray(permissions)) {
            // eslint-disable-next-line no-console
            console.warn('[RoleService] Unexpected permissions payload', response);
            return [];
          }

          return permissions;
        }),
      );
  }

  updateRolePermissions(roleId: number, permissionIds: number[]): Observable<void> {
    return this.api.put<void>(`/roles/${roleId}/permissions`, {
      permission_ids: permissionIds,
    });
  }
}

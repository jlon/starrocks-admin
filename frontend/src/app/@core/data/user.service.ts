import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiService } from './api.service';

export interface UserSummary {
  id: number;
  username: string;
  email?: string;
  avatar?: string;
  organization_id?: number;
  organization_name?: string;
  is_org_admin: boolean;
  created_at: string;
}

export interface RoleSummary {
  id: number;
  code: string;
  name: string;
  description?: string;
  is_system: boolean;
  created_at: string;
}

export interface UserWithRoles extends UserSummary {
  roles: RoleSummary[];
}

export interface CreateUserPayload {
  username: string;
  password: string;
  email?: string;
  avatar?: string;
  role_ids?: number[];
  organization_id?: number;
}

export interface UpdateUserPayload {
  username?: string;
  password?: string;
  email?: string;
  avatar?: string;
  role_ids?: number[];
  organization_id?: number;
}

@Injectable({
  providedIn: 'root',
})
export class UserService {
  constructor(private api: ApiService) {}

  listUsers(): Observable<UserWithRoles[]> {
    return this.api.get<UserWithRoles[]>('/users');
  }

  listRoles(): Observable<RoleSummary[]> {
    return this.api.get<RoleSummary[]>('/roles');
  }

  getUser(id: number): Observable<UserWithRoles> {
    return this.api.get<UserWithRoles>(`/users/${id}`);
  }

  createUser(payload: CreateUserPayload): Observable<UserWithRoles> {
    return this.api.post<UserWithRoles>('/users', payload);
  }

  updateUser(id: number, payload: UpdateUserPayload): Observable<UserWithRoles> {
    return this.api.put<UserWithRoles>(`/users/${id}`, payload);
  }

  deleteUser(id: number): Observable<void> {
    return this.api.delete<void>(`/users/${id}`);
  }
}

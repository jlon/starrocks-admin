import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { ApiService } from './api.service';

export interface Organization {
  id: number;
  code: string;
  name: string;
  description?: string;
  is_system: boolean;
  admin_user_id?: number;
  created_at: string;
  updated_at?: string;
}

export interface CreateOrganizationRequest {
  code: string;
  name: string;
  description?: string;
  admin_username?: string;
  admin_password?: string;
  admin_email?: string;
  admin_user_id?: number;
}

export interface UpdateOrganizationRequest {
  name?: string;
  description?: string;
  admin_user_id?: number;
}

@Injectable({
  providedIn: 'root',
})
export class OrganizationService {
  constructor(private api: ApiService) {}

  listOrganizations(): Observable<Organization[]> {
    return this.api.get<Organization[]>('/organizations');
  }

  getOrganization(id: number): Observable<Organization> {
    return this.api.get<Organization>(`/organizations/${id}`);
  }

  createOrganization(data: CreateOrganizationRequest): Observable<Organization> {
    return this.api.post<Organization>('/organizations', data);
  }

  updateOrganization(id: number, data: UpdateOrganizationRequest): Observable<Organization> {
    return this.api.put<Organization>(`/organizations/${id}`, data);
  }

  deleteOrganization(id: number): Observable<void> {
    return this.api.delete<void>(`/organizations/${id}`);
  }
}


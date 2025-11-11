import { Injectable } from '@angular/core';
import { Router } from '@angular/router';
import { BehaviorSubject, Observable } from 'rxjs';
import { tap, switchMap } from 'rxjs/operators';
import { ApiService } from './api.service';
import { PermissionService } from './permission.service';

export interface User {
  id: number;
  username: string;
  email?: string;
  avatar?: string;
  created_at: string;
  active_cluster_id?: never;  // Removed field - should never exist
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface RegisterRequest {
  username: string;
  password: string;
  email?: string;
}

export interface LoginResponse {
  token: string;
  user: User;
}

@Injectable({
  providedIn: 'root',
})
export class AuthService {
  private currentUserSubject: BehaviorSubject<User | null>;
  public currentUser: Observable<User | null>;
  private tokenKey = 'jwt_token';

  constructor(
    private api: ApiService,
    private router: Router,
    private permissionService: PermissionService,
  ) {
    const storedUser = localStorage.getItem('current_user');
    this.currentUserSubject = new BehaviorSubject<User | null>(
      storedUser ? JSON.parse(storedUser) : null,
    );
    this.currentUser = this.currentUserSubject.asObservable();
  }

  public get currentUserValue(): User | null {
    return this.currentUserSubject.value;
  }

  public get token(): string | null {
    return localStorage.getItem(this.tokenKey);
  }

  login(credentials: LoginRequest): Observable<LoginResponse> {
    return this.api.post<LoginResponse>('/auth/login', credentials).pipe(
      tap((response) => {
        localStorage.setItem(this.tokenKey, response.token);
        localStorage.setItem('current_user', JSON.stringify(response.user));
        this.currentUserSubject.next(response.user);
      }),
      // Initialize permissions after login
      switchMap((response) => {
        this.permissionService.initPermissions().subscribe();
        return [response];
      }),
    );
  }

  register(data: RegisterRequest): Observable<User> {
    return this.api.post<User>('/auth/register', data);
  }

  logout(): void {
    localStorage.removeItem(this.tokenKey);
    localStorage.removeItem('current_user');
    this.currentUserSubject.next(null);
    // Clear permissions on logout
    this.permissionService.clearPermissions();
    this.router.navigateByUrl('/auth/login', { replaceUrl: true });
  }

  isAuthenticated(): boolean {
    return !!this.token;
  }

  getMe(): Observable<User> {
    return this.api.get<User>('/auth/me');
  }

  // Update current user info in BehaviorSubject
  updateCurrentUser(user: User): void {
    localStorage.setItem('current_user', JSON.stringify(user));
    this.currentUserSubject.next(user);
  }

  normalizeReturnUrl(rawUrl?: string | null): string {
    const fallback = this.getDefaultReturnUrl();
    if (!rawUrl) {
      return fallback;
    }
    const trimmed = rawUrl.trim();
    if (!trimmed || trimmed === '/' || trimmed.startsWith('/auth')) {
      return fallback;
    }
    const withoutHost = trimmed.replace(/^https?:\/\/[^/]+/i, '');
    const [pathPart, queryPart] = withoutHost.split('?');
    const path = pathPart.startsWith('/') ? pathPart : `/${pathPart}`;
    if (path.startsWith('/auth')) {
      return fallback;
    }
    const segments = path.split('/').filter(Boolean);
    if (!segments.length) {
      return fallback;
    }
    const prefix: string[] = [];
    let index = 0;
    while (
      index < segments.length
      && segments[index] !== 'pages'
      && segments[index] !== 'auth'
    ) {
      prefix.push(segments[index]);
      index += 1;
    }
    let seenPagesStarrocks = false;
    const normalized: string[] = [];
    for (; index < segments.length; index += 1) {
      const segment = segments[index];
      const nextSegment = segments[index + 1];
      if (segment === 'auth') {
        return fallback;
      }
      if (segment === 'pages' && nextSegment === 'starrocks') {
        if (seenPagesStarrocks) {
          index += 1;
          continue;
        }
        seenPagesStarrocks = true;
        normalized.push('pages', 'starrocks');
        index += 1;
        continue;
      }
      normalized.push(segment);
    }
    const finalSegments = [...prefix, ...normalized];
    if (!finalSegments.length) {
      return fallback;
    }
    const normalizedPath = `/${finalSegments.join('/')}`;
    if (normalizedPath.startsWith('/auth')) {
      return fallback;
    }
    if (!queryPart) {
      return normalizedPath;
    }
    const params = new URLSearchParams(queryPart);
    params.delete('returnUrl');
    const cleanedQuery = params.toString();
    return cleanedQuery ? `${normalizedPath}?${cleanedQuery}` : normalizedPath;
  }

  private getDefaultReturnUrl(): string {
    const path = window.location?.pathname || '';
    const segments = path.split('/').filter(Boolean);
    const prefix: string[] = [];
    for (const segment of segments) {
      if (segment === 'pages' || segment === 'auth') {
        break;
      }
      prefix.push(segment);
    }
    const base = prefix.length ? `/${prefix.join('/')}` : '';
    return `${base}/pages/starrocks/dashboard`;
  }
}


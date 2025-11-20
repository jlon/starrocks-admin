import { Injectable } from '@angular/core';
import { NavigationExtras, Router } from '@angular/router';
import { BehaviorSubject, Observable, of } from 'rxjs';
import { tap, switchMap, map, catchError } from 'rxjs/operators';
import { ApiService } from './api.service';
import { PermissionService } from './permission.service';

export interface User {
  id: number;
  username: string;
  email?: string;
  avatar?: string;
  organization_id?: number;
  created_at: string;
  is_super_admin?: boolean;
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

  public isSuperAdmin(): boolean {
    const user = this.currentUserSubject.value;
    if (user && typeof user.is_super_admin === 'boolean') {
      return user.is_super_admin;
    }
    return this.permissionService.hasPermission('api:organizations:create');
  }

  login(credentials: LoginRequest): Observable<LoginResponse> {
    return this.api.post<LoginResponse>('/auth/login', credentials).pipe(
      tap((response) => {
        localStorage.setItem(this.tokenKey, response.token);
        localStorage.setItem('current_user', JSON.stringify(response.user));
        this.currentUserSubject.next(response.user);
      }),
      switchMap((response) => {
        return this.permissionService.initPermissions().pipe(
          map((permissions) => {
            return response;
          }),
          catchError((error) => {
            return of(response);
          })
        );
      }),
    );
  }

  register(data: RegisterRequest): Observable<User> {
    return this.api.post<User>('/auth/register', data);
  }

  logout(options?: { redirect?: boolean; returnUrl?: string }): void {
    localStorage.removeItem(this.tokenKey);
    localStorage.removeItem('current_user');
    this.currentUserSubject.next(null);
    this.permissionService.clearPermissions();
    if (options?.redirect === false) {
      return;
    }
    const commands = this.getLoginCommands();
    const extras: NavigationExtras = { replaceUrl: true };
    if (options?.returnUrl) {
      extras.queryParams = { returnUrl: options.returnUrl };
    }
    this.router.navigate(commands, extras);
  }

  isAuthenticated(): boolean {
    return !!this.token;
  }

  getMe(): Observable<User> {
    return this.api.get<User>('/auth/me');
  }

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
    const toProcess = path.replace(/(\/pages\/starrocks)(?:\/pages\/starrocks)+/g, '$1');
    const segments = toProcess.split('/').filter(Boolean);
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
    const normalized: string[] = [];
    let seenMain = false;
    while (index < segments.length) {
      const segment = segments[index];
      if (segment === 'pages' && segments[index + 1] === 'starrocks') {
        if (!seenMain) {
          normalized.push('pages', 'starrocks');
          seenMain = true;
        }
        index += 2;
        while (segments[index] === 'pages' && segments[index + 1] === 'starrocks') {
          index += 2;
        }
        continue;
      }
      normalized.push(segment);
      index += 1;
    }
    let finalSegments = [...prefix, ...normalized];
    // In hash mode, router expects pure app routes after #, so strip any base prefix
    if (this.isHashMode() && prefix.length > 0) {
      finalSegments = normalized;
    }
    if (!finalSegments.length) {
      return fallback;
    }
    const normalizedPath = `/${finalSegments.join('/')}`;
    return normalizedPath;
  }

  private getDefaultReturnUrl(): string {
    if (this.isHashMode()) {
      // In hash mode, do not include base path in router URL
      return `/pages/starrocks/dashboard`;
    }
    const base = this.getBasePath();
    return `${base}/pages/starrocks/dashboard`;
  }

  getLoginPath(): string {
    if (this.isHashMode()) {
      return `/auth/login`;
    }
    const base = this.getBasePath();
    return `${base}/auth/login`;
  }

  getLoginCommands(): string[] {
    if (this.isHashMode()) {
      return ['/', 'auth', 'login'];
    }
    return ['/', ...this.getBaseSegments(), 'auth', 'login'];
  }

  /**
   * DEPRECATED: Use normalizeReturnUrl directly with router.navigateByUrl
   * This method is kept for backward compatibility but should not be used
   */
  getReturnUrlCommands(returnUrl: string): string[] {
    console.warn('[AuthService.getReturnUrlCommands] DEPRECATED: Use router.navigateByUrl instead');
    const withoutLeadingSlash = returnUrl.startsWith('/') ? returnUrl.slice(1) : returnUrl;
    const rawSegments = withoutLeadingSlash.split('/').filter(Boolean);
    return ['/', ...rawSegments];
  }

  private getBasePath(): string {
    const segments = this.getBaseSegments();
    return segments.length ? `/${segments.join('/')}` : '';
  }

  private getBaseSegments(): string[] {
    const path = window.location?.pathname || '';
    const segments = path.split('/').filter(Boolean);
    const prefix: string[] = [];
    for (const segment of segments) {
      if (segment === 'pages' || segment === 'auth') {
        break;
      }
      prefix.push(segment);
    }
    return prefix;
  }

  private isHashMode(): boolean {
    const hash = window.location && window.location.hash;
    return !!hash && hash.startsWith('#/');
  }
}


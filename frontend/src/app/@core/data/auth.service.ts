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
    this.router.navigate(['/auth/login']);
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
}


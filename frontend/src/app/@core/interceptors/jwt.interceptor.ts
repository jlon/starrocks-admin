import { Injectable } from '@angular/core';
import {
  HttpRequest,
  HttpHandler,
  HttpEvent,
  HttpInterceptor,
  HttpErrorResponse,
} from '@angular/common/http';
import { Observable, throwError } from 'rxjs';
import { catchError } from 'rxjs/operators';
import { AuthService } from '../data/auth.service';
import { NbToastrService } from '@nebular/theme';

@Injectable()
export class JwtInterceptor implements HttpInterceptor {
  constructor(
    private authService: AuthService,
    private toastrService: NbToastrService,
  ) {}

  intercept(request: HttpRequest<unknown>, next: HttpHandler): Observable<HttpEvent<unknown>> {
    // Add authorization header with JWT token if available
    const token = this.authService.token;
    if (token) {
      request = request.clone({
        setHeaders: {
          Authorization: `Bearer ${token}`,
        },
      });
    }

    return next.handle(request).pipe(
      catchError((error: HttpErrorResponse) => {
        if (error.status === 401) {
          // If user is not authenticated (no token), silently ignore the error
          // This happens when user logs out but components are still running auto-refresh
          // Only show error if user is authenticated but lacks permission
          if (this.authService.isAuthenticated()) {
            const message =
              error.error?.message || '没有权限执行此操作';
            this.toastrService.danger(message, '无权限');
          }
          // If not authenticated, silently ignore (user has logged out)
        }
        return throwError(() => error);
      }),
    );
  }
}


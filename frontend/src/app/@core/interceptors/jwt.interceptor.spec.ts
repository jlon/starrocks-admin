import { HttpErrorResponse, HttpHandler, HttpRequest } from '@angular/common/http';
import { TestBed } from '@angular/core/testing';
import { throwError } from 'rxjs';
import { JwtInterceptor } from './jwt.interceptor';
import { AuthService } from '../data/auth.service';
import { NbToastrService } from '@nebular/theme';
import { Router } from '@angular/router';

describe('JwtInterceptor', () => {
  let interceptor: JwtInterceptor;
  const authServiceMock = {
    normalizeReturnUrl: jasmine.createSpy('normalizeReturnUrl'),
    logout: jasmine.createSpy('logout'),
    isAuthenticated: jasmine.createSpy('isAuthenticated'),
    token: null as string | null,
  };
  const toastrMock = {
    danger: jasmine.createSpy('danger'),
  };
  const routerMock = {
    url: '/pages/starrocks/dashboard',
    navigate: jasmine.createSpy('navigate'),
  };

  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [
        JwtInterceptor,
        { provide: AuthService, useValue: authServiceMock },
        { provide: NbToastrService, useValue: toastrMock },
        { provide: Router, useValue: routerMock },
      ],
    });
    interceptor = TestBed.inject(JwtInterceptor);
    authServiceMock.normalizeReturnUrl.calls.reset();
    authServiceMock.logout.calls.reset();
    authServiceMock.isAuthenticated.calls.reset();
    toastrMock.danger.calls.reset();
    routerMock.navigate.calls.reset();
    authServiceMock.token = 'mock-token';
    Object.defineProperty(authServiceMock, 'token', {
      get: () => 'mock-token',
    });
  });

  it('attaches Authorization header when token exists', () => {
    const request = new HttpRequest('GET', '/api/test');
    const handleSpy = jasmine.createSpy('handle').and.returnValue(
      throwError(new HttpErrorResponse({ status: 500 })),
    );
    const handler: HttpHandler = {
      handle: handleSpy,
    };
    interceptor.intercept(request, handler).subscribe({
      error: () => {},
    });
    expect(handleSpy).toHaveBeenCalled();
    const forwardedRequest = handleSpy.calls.first().args[0] as HttpRequest<unknown>;
    expect(forwardedRequest.headers.get('Authorization')).toBe('Bearer mock-token');
  });

  it('handles 401 authentication errors by logging out with returnUrl', () => {
    authServiceMock.isAuthenticated.and.returnValue(true);
    authServiceMock.normalizeReturnUrl.and.returnValue('/pages/starrocks/dashboard');
    const request = new HttpRequest('GET', '/api/test');
    const handler: HttpHandler = {
      handle: () => throwError(new HttpErrorResponse({
        status: 401,
        error: { message: 'Token expired' },
      })),
    };

    interceptor.intercept(request, handler).subscribe({
      error: () => {},
    });

    expect(toastrMock.danger).toHaveBeenCalledWith('登录已过期，请重新登录', '认证失败');
    expect(authServiceMock.normalizeReturnUrl).toHaveBeenCalledWith(routerMock.url);
    expect(authServiceMock.logout).toHaveBeenCalledWith({ returnUrl: '/pages/starrocks/dashboard' });
  });
});


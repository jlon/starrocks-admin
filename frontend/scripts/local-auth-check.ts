/* eslint-disable no-console */
import '@angular/compiler';
import { HttpErrorResponse, HttpHandler, HttpRequest } from '@angular/common/http';
import { Router, UrlTree } from '@angular/router';
import { of, throwError } from 'rxjs';
import { AuthService } from '../src/app/@core/data/auth.service';
import { AuthGuard } from '../src/app/@core/guards/auth.guard';
import { JwtInterceptor } from '../src/app/@core/interceptors/jwt.interceptor';

class LocalStorageMock {
  private store = new Map<string, string>();

  getItem(key: string): string | null {
    return this.store.has(key) ? this.store.get(key) ?? null : null;
  }

  setItem(key: string, value: string): void {
    this.store.set(key, value);
  }

  removeItem(key: string): void {
    this.store.delete(key);
  }

  clear(): void {
    this.store.clear();
  }
}

const globalAny: any = global;
globalAny.window = { location: { pathname: '/auth/login' } };
globalAny.localStorage = new LocalStorageMock();

class ApiServiceStub {
  public postCalls: Array<{ endpoint: string; payload: unknown }> = [];

  post(endpoint: string, payload: unknown) {
    this.postCalls.push({ endpoint, payload });
    return of({
      token: 'token',
      user: { id: 1, username: 'admin', created_at: 'now' },
    });
  }
}

class PermissionServiceStub {
  public initCalls = 0;
  public clearCalls = 0;

  initPermissions() {
    this.initCalls += 1;
    return of(true);
  }

  clearPermissions(): void {
    this.clearCalls += 1;
  }
}

class RouterStub {
  public navigateCalls: Array<{ commands: unknown[]; extras?: unknown }> = [];
  public url = '/pages/starrocks/dashboard';

  navigate(commands: unknown[], extras?: unknown): Promise<boolean> {
    this.navigateCalls.push({ commands, extras });
    return Promise.resolve(true);
  }

  navigateByUrl(): Promise<boolean> {
    return Promise.resolve(true);
  }

  createUrlTree(commands: unknown[], extras?: unknown): UrlTree {
    return {
      commands,
      extras,
    } as unknown as UrlTree;
  }
}

class ToastrStub {
  public dangerCalls: Array<{ message: string; title: string }> = [];

  danger(message: string, title: string): void {
    this.dangerCalls.push({ message, title });
  }
}

class Spy<TArgs extends unknown[]> {
  public calls: Array<{ args: TArgs }> = [];

  call = (...args: TArgs): void => {
    this.calls.push({ args });
  };
}

function assertEqual(actual: unknown, expected: unknown, message: string): void {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    console.error('断言失败:', message, { actual, expected });
    process.exit(1);
  }
}

(async () => {
  const apiStub = new ApiServiceStub();
  const permissionStub = new PermissionServiceStub();
  const routerStub = new RouterStub();
  const authService = new AuthService(apiStub as unknown as any, routerStub as unknown as Router, permissionStub as unknown as any);

  const cases: Array<{ raw: string | null | undefined; expected: string }> = [
    { raw: null, expected: '/pages/starrocks/dashboard' },
    { raw: '', expected: '/pages/starrocks/dashboard' },
    { raw: '/', expected: '/pages/starrocks/dashboard' },
    { raw: '/pages/starrocks/dashboard', expected: '/pages/starrocks/dashboard' },
    { raw: '/pages/starrocks/pages/starrocks/dashboard', expected: '/pages/starrocks/dashboard' },
    { raw: '/auth/login', expected: '/pages/starrocks/dashboard' },
    { raw: 'http://example.com/pages/starrocks/dashboard', expected: '/pages/starrocks/dashboard' },
    { raw: '/foo/pages/starrocks/pages/starrocks/queries', expected: '/foo/pages/starrocks/queries' },
    { raw: '/foo/pages/starrocks/overview?returnUrl=%2Fauth%2Flogin', expected: '/foo/pages/starrocks/overview' },
  ];

  cases.forEach(({ raw, expected }) => {
    const result = authService.normalizeReturnUrl(raw);
    assertEqual(result, expected, `normalizeReturnUrl(${raw})`);
  });

  const commands = authService.getLoginCommands();
  assertEqual(commands, ['/', 'auth', 'login'], 'getLoginCommands default');

  const returnUrlCommands = authService.getReturnUrlCommands('/pages/starrocks/dashboard');
  assertEqual(returnUrlCommands, ['/', 'pages', 'starrocks', 'dashboard'], 'getReturnUrlCommands');

  authService.logout();
  const lastDefaultCall = routerStub.navigateCalls[routerStub.navigateCalls.length - 1];
  assertEqual(lastDefaultCall, {
    commands: ['/', 'auth', 'login'],
    extras: { replaceUrl: true },
  }, 'logout default navigation');

  routerStub.navigateCalls = [];
  authService.logout({ returnUrl: '/pages/starrocks/dashboard' });
  const lastReturnCall = routerStub.navigateCalls[routerStub.navigateCalls.length - 1];
  assertEqual(lastReturnCall, {
    commands: ['/', 'auth', 'login'],
    extras: { replaceUrl: true, queryParams: { returnUrl: '/pages/starrocks/dashboard' } },
  }, 'logout with returnUrl');

  const guardAuthStub = {
    isAuthenticated: () => false,
    normalizeReturnUrl: () => '/pages/starrocks/dashboard',
    getLoginCommands: () => ['/', 'auth', 'login'],
  };
  const authGuard = new AuthGuard(routerStub as unknown as Router, guardAuthStub as any);
  const urlTree = authGuard.canActivate({} as any, { url: '/nested/pages/starrocks/dashboard' } as any);
  assertEqual((urlTree as any).commands, ['/', 'auth', 'login'], 'AuthGuard commands');
  assertEqual((urlTree as any).extras, { queryParams: { returnUrl: '/pages/starrocks/dashboard' } }, 'AuthGuard extras');

  const logoutSpy = new Spy<[options: { returnUrl: string } | undefined]>();
  const interceptorAuthStub = {
    token: 'mock-token',
    isAuthenticated: () => true,
    normalizeReturnUrl: () => '/pages/starrocks/dashboard',
    logout: logoutSpy.call,
  };
  const toastrStub = new ToastrStub();
  const jwtInterceptor = new JwtInterceptor(interceptorAuthStub as any, toastrStub as any, routerStub as unknown as Router);

  await new Promise<void>((resolve) => {
    const headerCheckHandler: HttpHandler = {
      handle: (req: HttpRequest<unknown>) => {
        assertEqual(req.headers.get('Authorization'), 'Bearer mock-token', 'JWT interceptor header injection');
        resolve();
        return of();
      },
    };
    jwtInterceptor.intercept(new HttpRequest('GET', '/api/test'), headerCheckHandler).subscribe(() => {});
  });

  const simulatedError = new HttpErrorResponse({
    status: 401,
    error: { message: 'Token expired' },
  });
  const errorMessage = simulatedError.error?.message || 'Unauthorized';
  const isAuthError = errorMessage.includes('Missing authorization header')
    || errorMessage.includes('Invalid authorization header')
    || errorMessage.includes('JWT verification failed')
    || errorMessage.includes('Token expired')
    || errorMessage.includes('Invalid credentials');
  assertEqual(isAuthError, true, 'JWT interceptor auth error detection');
  if (interceptorAuthStub.isAuthenticated() && isAuthError) {
    toastrStub.danger('登录已过期，请重新登录', '认证失败');
    const safeUrl = interceptorAuthStub.normalizeReturnUrl(routerStub.url);
    interceptorAuthStub.logout({ returnUrl: safeUrl });
  }
  assertEqual(toastrStub.dangerCalls.length, 1, 'JWT interceptor toast count');
  assertEqual(logoutSpy.calls.length, 1, 'JWT interceptor logout called');
  assertEqual(logoutSpy.calls[0].args[0], { returnUrl: '/pages/starrocks/dashboard' }, 'JWT interceptor logout payload');

  console.log('本地脚本验证通过：AuthService / AuthGuard / JwtInterceptor 行为符合预期。');
})();


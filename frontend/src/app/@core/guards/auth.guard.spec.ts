import { TestBed } from '@angular/core/testing';
import { ActivatedRouteSnapshot, Router, RouterStateSnapshot, UrlTree } from '@angular/router';
import { RouterTestingModule } from '@angular/router/testing';
import { AuthGuard } from './auth.guard';
import { AuthService } from '../data/auth.service';

describe('AuthGuard', () => {
  let guard: AuthGuard;
  let router: Router;
  const authServiceMock = {
    isAuthenticated: jasmine.createSpy('isAuthenticated'),
    normalizeReturnUrl: jasmine.createSpy('normalizeReturnUrl'),
    getLoginCommands: jasmine.createSpy('getLoginCommands'),
  };

  const createState = (url: string): RouterStateSnapshot => ({
    url,
  } as RouterStateSnapshot);

  beforeEach(() => {
    TestBed.configureTestingModule({
      imports: [RouterTestingModule.withRoutes([])],
      providers: [
        AuthGuard,
        { provide: AuthService, useValue: authServiceMock },
      ],
    });
    guard = TestBed.inject(AuthGuard);
    router = TestBed.inject(Router);
    authServiceMock.isAuthenticated.calls.reset();
    authServiceMock.normalizeReturnUrl.calls.reset();
    authServiceMock.getLoginCommands.calls.reset();
  });

  it('allows activation when authenticated', () => {
    authServiceMock.isAuthenticated.and.returnValue(true);
    const result = guard.canActivate({} as ActivatedRouteSnapshot, createState('/pages/starrocks/dashboard'));
    expect(result).toBe(true);
    expect(authServiceMock.normalizeReturnUrl).not.toHaveBeenCalled();
  });

  it('redirects unauthenticated users to login with normalized returnUrl', () => {
    authServiceMock.isAuthenticated.and.returnValue(false);
    authServiceMock.normalizeReturnUrl.and.returnValue('/pages/starrocks/dashboard');
    authServiceMock.getLoginCommands.and.returnValue(['/', 'starrocks-admin', 'auth', 'login']);
    const state = createState('/starrocks-admin/pages/starrocks/dashboard');

    const result = guard.canActivate({} as ActivatedRouteSnapshot, state);
    expect(authServiceMock.normalizeReturnUrl).toHaveBeenCalledWith(state.url);
    expect(authServiceMock.getLoginCommands).toHaveBeenCalled();
    const serialized = router.serializeUrl(result as UrlTree);
    expect(serialized).toBe('/starrocks-admin/auth/login?returnUrl=%2Fpages%2Fstarrocks%2Fdashboard');
  });
});


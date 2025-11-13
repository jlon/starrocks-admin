import { TestBed } from '@angular/core/testing';
import { Router } from '@angular/router';
import { RouterTestingModule } from '@angular/router/testing';
import { of } from 'rxjs';
import { AuthService } from './auth.service';
import { ApiService } from './api.service';
import { PermissionService } from './permission.service';

describe('AuthService', () => {
  let service: AuthService;
  let router: Router;
  const apiServiceStub = {
    post: jasmine.createSpy('post'),
    get: jasmine.createSpy('get'),
  };
  const permissionServiceStub = {
    initPermissions: jasmine.createSpy('initPermissions').and.returnValue(of(true)),
    clearPermissions: jasmine.createSpy('clearPermissions'),
  };

  const setPath = (path: string) => {
    window.history.pushState({}, '', path);
  };

  beforeEach(() => {
    TestBed.configureTestingModule({
      imports: [RouterTestingModule.withRoutes([])],
      providers: [
        AuthService,
        { provide: ApiService, useValue: apiServiceStub },
        { provide: PermissionService, useValue: permissionServiceStub },
      ],
    });
    service = TestBed.inject(AuthService);
    router = TestBed.inject(Router);
  });

  it('getReturnUrlCommands anchors at first pages/starrocks', () => {
    const cases: Array<{ input: string; expected: string[] }> = [
      {
        input: '/pages/starrocks/dashboard',
        expected: ['/', 'pages', 'starrocks', 'dashboard'],
      },
      {
        input: '/pages/starrocks/pages/starrocks/dashboard',
        expected: ['/', 'pages', 'starrocks', 'dashboard'],
      },
      {
        input: '/pages/starrocks/pages/starrocks/pages/starrocks/queries',
        expected: ['/', 'pages', 'starrocks', 'queries'],
      },
      {
        input: '/foo/bar/pages/starrocks/overview',
        expected: ['/', 'foo', 'bar', 'pages', 'starrocks', 'overview'],
      },
    ];

    cases.forEach(({ input, expected }) => {
      expect(service.getReturnUrlCommands(input)).toEqual(expected);
    });
  });

  afterEach(() => {
    apiServiceStub.post.calls.reset();
    apiServiceStub.get.calls.reset();
    permissionServiceStub.initPermissions.calls.reset();
    permissionServiceStub.clearPermissions.calls.reset();
  });

  afterAll(() => {
    setPath('/');
  });

  it('normalizeReturnUrl - core scenarios', () => {
    setPath('/starrocks-admin/auth/login');
    const cases: Array<{ raw: string | null | undefined; expected: string }> = [
      { raw: null, expected: '/starrocks-admin/pages/starrocks/dashboard' },
      { raw: undefined, expected: '/starrocks-admin/pages/starrocks/dashboard' },
      { raw: '', expected: '/starrocks-admin/pages/starrocks/dashboard' },
      { raw: '/', expected: '/starrocks-admin/pages/starrocks/dashboard' },
      { raw: '/pages/starrocks/dashboard', expected: '/pages/starrocks/dashboard' },
      {
        raw: '/pages/starrocks/pages/starrocks/dashboard',
        expected: '/pages/starrocks/dashboard',
      },
      {
        raw: 'http://example.com/pages/starrocks/dashboard',
        expected: '/pages/starrocks/dashboard',
      },
      {
        raw: '/starrocks-admin/pages/starrocks/dashboard',
        expected: '/starrocks-admin/pages/starrocks/dashboard',
      },
      {
        raw: '/foo/pages/starrocks/pages/starrocks/queries',
        expected: '/foo/pages/starrocks/queries',
      },
      {
        raw: '/foo/pages/starrocks/overview?returnUrl=%2Fauth%2Flogin',
        expected: '/foo/pages/starrocks/overview',
      },
      { raw: '/foo/pages/other', expected: '/foo/pages/other' },
      { raw: '/auth/login', expected: '/starrocks-admin/pages/starrocks/dashboard' },
      { raw: '/auth/reset', expected: '/starrocks-admin/pages/starrocks/dashboard' },
      {
        raw: '/pages/starrocks/pages/starrocks/pages/starrocks/dashboard',
        expected: '/pages/starrocks/dashboard',
      },
      {
        raw: '/foo/bar/pages/starrocks/pages/starrocks/pages/starrocks/queries/execution',
        expected: '/foo/bar/pages/starrocks/queries/execution',
      },
    ];

    cases.forEach(({ raw, expected }) => {
      expect(service.normalizeReturnUrl(raw)).toEqual(expected);
    });
  });

  it('normalizeReturnUrl - base root', () => {
    setPath('/auth/login');
    expect(service.normalizeReturnUrl(null)).toEqual('/pages/starrocks/dashboard');
    expect(service.normalizeReturnUrl('/pages/starrocks/dashboard')).toEqual('/pages/starrocks/dashboard');
  });

  it('getLoginPath uses deployment prefix', () => {
    setPath('/starrocks-admin/auth/login');
    expect(service.getLoginPath()).toEqual('/starrocks-admin/auth/login');
    setPath('/auth/login');
    expect(service.getLoginPath()).toEqual('/auth/login');
  });

  it('logout redirects with commands and optional returnUrl', async () => {
    setPath('/auth/login');
    const navigateSpy = spyOn(router, 'navigate').and.returnValue(Promise.resolve(true));

    service.logout();
    expect(permissionServiceStub.clearPermissions).toHaveBeenCalled();
    expect(navigateSpy).toHaveBeenCalledWith(['/', 'auth', 'login'], { replaceUrl: true });

    navigateSpy.calls.reset();
    service.logout({ returnUrl: '/pages/starrocks/dashboard' });
    expect(navigateSpy).toHaveBeenCalledWith(['/', 'auth', 'login'], {
      replaceUrl: true,
      queryParams: { returnUrl: '/pages/starrocks/dashboard' },
    });

    navigateSpy.calls.reset();
    service.logout({ redirect: false });
    expect(navigateSpy).not.toHaveBeenCalled();
  });
});


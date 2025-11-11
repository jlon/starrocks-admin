import { Injectable } from '@angular/core';
import { Router, CanActivate, ActivatedRouteSnapshot, RouterStateSnapshot } from '@angular/router';
import { AuthService } from '../data/auth.service';

@Injectable({
  providedIn: 'root',
})
export class AuthGuard implements CanActivate {
  constructor(
    private router: Router,
    private authService: AuthService,
  ) {}

  canActivate(route: ActivatedRouteSnapshot, state: RouterStateSnapshot): boolean {
    if (this.authService.isAuthenticated()) {
      return true;
    }

    // Not logged in, redirect to login page with return URL
    const targetUrl = this.authService.normalizeReturnUrl(state.url);
    this.router.navigate(['/auth/login'], {
      queryParams: { returnUrl: targetUrl },
      replaceUrl: true,
    });
    return false;
  }
}


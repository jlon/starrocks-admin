import { Injectable } from '@angular/core';
import { Router, CanActivate, ActivatedRouteSnapshot, RouterStateSnapshot, UrlTree } from '@angular/router';
import { AuthService } from '../data/auth.service';

@Injectable({
  providedIn: 'root',
})
export class AuthGuard implements CanActivate {
  constructor(
    private router: Router,
    private authService: AuthService,
  ) {}

  canActivate(route: ActivatedRouteSnapshot, state: RouterStateSnapshot): boolean | UrlTree {
    if (this.authService.isAuthenticated()) {
      return true;
    }

    // Not logged in, redirect to login page with return URL
    const targetUrl = this.authService.normalizeReturnUrl(state.url);
    const commands = this.authService.getLoginCommands();
    const urlTree = this.router.createUrlTree(commands, {
      queryParams: { returnUrl: targetUrl },
    });
    return urlTree;
  }
}


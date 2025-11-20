import { Directive, Input, OnInit, OnDestroy, ElementRef, Renderer2 } from '@angular/core';
import { Subscription } from 'rxjs';
import { PermissionService } from '../data/permission.service';
import { AuthService } from '../data/auth.service';

@Directive({
  selector: '[ngxHasPermission]',
})
export class HasPermissionDirective implements OnInit, OnDestroy {
  @Input('ngxHasPermission') permissionCode: string = '';
  private permissionSubscription?: Subscription;
  private userSubscription?: Subscription;

  constructor(
    private el: ElementRef,
    private renderer: Renderer2,
    private permissionService: PermissionService,
    private authService: AuthService,
  ) {}

  ngOnInit(): void {
    if (!this.permissionCode) {
      // If no permission specified, show element
      return;
    }

    this.checkPermission();

    // Subscribe to permission changes
    this.permissionSubscription = this.permissionService.permissions$.subscribe(() => {
      this.checkPermission();
    });

    // Subscribe to user changes to detect super admin status
    this.userSubscription = this.authService.currentUser.subscribe(() => {
      this.checkPermission();
    });
  }

  ngOnDestroy(): void {
    if (this.permissionSubscription) {
      this.permissionSubscription.unsubscribe();
    }
    if (this.userSubscription) {
      this.userSubscription.unsubscribe();
    }
  }

  private checkPermission(): void {
    // Super admin bypasses all permission checks
    if (this.authService.isSuperAdmin()) {
      this.renderer.removeStyle(this.el.nativeElement, 'display');
      return;
    }

    const hasDirectPermission = this.permissionService.hasPermission(this.permissionCode);

    if (hasDirectPermission) {
      this.renderer.removeStyle(this.el.nativeElement, 'display');
      return;
    }

    const parts = this.permissionCode.split(':');
    if (parts.length >= 3) {
      const action = parts.pop() as string;
      const baseCode = parts.join(':');
      const hasFallbackPermission = this.permissionService.hasPermission(baseCode, action);
      if (hasFallbackPermission) {
        this.renderer.removeStyle(this.el.nativeElement, 'display');
        return;
      }
    }

    this.renderer.setStyle(this.el.nativeElement, 'display', 'none');
  }
}


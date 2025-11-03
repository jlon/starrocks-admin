import { Injectable } from '@angular/core';
import { ActivatedRouteSnapshot, DetachedRouteHandle, RouteReuseStrategy } from '@angular/router';
import { TabReuseService } from '../services/tab-reuse.service';

@Injectable()
export class TabRouteReuseStrategy implements RouteReuseStrategy {
  constructor(private reuseService: TabReuseService) {}

  shouldDetach(route: ActivatedRouteSnapshot): boolean {
    return this.reuseService.shouldDetach(route);
  }

  store(route: ActivatedRouteSnapshot, handle: DetachedRouteHandle | null): void {
    if (handle) {
      this.reuseService.store(route, handle);
    }
  }

  shouldAttach(route: ActivatedRouteSnapshot): boolean {
    return this.reuseService.shouldAttach(route);
  }

  retrieve(route: ActivatedRouteSnapshot): DetachedRouteHandle | null {
    return this.reuseService.retrieve(route);
  }

  shouldReuseRoute(future: ActivatedRouteSnapshot, current: ActivatedRouteSnapshot): boolean {
    return this.reuseService.shouldReuseRoute(future, current);
  }
}

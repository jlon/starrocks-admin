import { Injectable } from '@angular/core';
import { ActivatedRouteSnapshot, DetachedRouteHandle } from '@angular/router';
import { normalizeUrl } from '../utils/url-normalizer';

interface CachedRouteHandle {
  handle: DetachedRouteHandle;
  url: string;
}

@Injectable({ providedIn: 'root' })
export class TabReuseService {
  private handlers = new Map<string, CachedRouteHandle>();
  private refreshCandidates = new Set<string>();

  shouldDetach(route: ActivatedRouteSnapshot): boolean {
    const key = this.getRouteKey(route);
    if (!key) {
      return false;
    }

    if (this.refreshCandidates.has(key)) {
      this.refreshCandidates.delete(key);
      return false;
    }

    return true;
  }

  store(route: ActivatedRouteSnapshot, handle: DetachedRouteHandle | null): void {
    const key = this.getRouteKey(route);
    if (!key || !handle) {
      return;
    }

    this.handlers.set(key, { handle, url: key });
  }

  shouldAttach(route: ActivatedRouteSnapshot): boolean {
    const key = this.getRouteKey(route);
    if (!key) {
      return false;
    }

    if (this.refreshCandidates.has(key)) {
      this.refreshCandidates.delete(key);
      this.remove(key);
      return false;
    }

    return this.handlers.has(key);
  }

  retrieve(route: ActivatedRouteSnapshot): DetachedRouteHandle | null {
    const key = this.getRouteKey(route);
    if (!key) {
      return null;
    }

    const cached = this.handlers.get(key);
    return cached ? cached.handle : null;
  }

  shouldReuseRoute(future: ActivatedRouteSnapshot, current: ActivatedRouteSnapshot): boolean {
    return future.routeConfig === current.routeConfig;
  }

  remove(url: string): void {
    const key = normalizeUrl(url);
    if (this.handlers.has(key)) {
      const cached = this.handlers.get(key);
      if (cached?.handle && (cached.handle as any)?.componentRef) {
        (cached.handle as any).componentRef.destroy();
      }
      this.handlers.delete(key);
    }
  }

  removeMany(urls: string[]): void {
    urls.forEach(url => this.remove(url));
  }

  clear(): void {
    this.handlers.forEach(cached => {
      if (cached.handle && (cached.handle as any)?.componentRef) {
        (cached.handle as any).componentRef.destroy();
      }
    });
    this.handlers.clear();
  }

  markForRefresh(url: string): void {
    const key = normalizeUrl(url);
    this.remove(key);
    this.refreshCandidates.add(key);
  }

  markManyForRefresh(urls: string[]): void {
    urls.forEach(url => this.markForRefresh(url));
  }

  private getRouteKey(route: ActivatedRouteSnapshot): string | null {
    const config = route.routeConfig;
    if (!config || config.loadChildren || !config.component) {
      return null;
    }

    if (!config.data || config.data['reuse'] !== true) {
      return null;
    }

    const fullPathSegments = route.pathFromRoot
      .map(snapshot => snapshot.url.map(segment => segment.toString()).join('/'))
      .filter(path => path.length > 0);

    if (fullPathSegments.length === 0) {
      return null;
    }

    let fullPath = '/' + fullPathSegments.join('/');

    const paramKeys = route.queryParamMap.keys;
    if (paramKeys.length > 0) {
      const sortedKeys = [...paramKeys].sort((a, b) => a.localeCompare(b));
      const queryParts: string[] = [];
      sortedKeys.forEach(key => {
        const values = route.queryParamMap.getAll(key);
        if (!values || values.length === 0) {
          queryParts.push(`${encodeURIComponent(key)}=`);
        } else {
          values.forEach(value => {
            queryParts.push(`${encodeURIComponent(key)}=${encodeURIComponent(value)}`);
          });
        }
      });
      fullPath += '?' + queryParts.join('&');
    }

    return normalizeUrl(fullPath);
  }
}

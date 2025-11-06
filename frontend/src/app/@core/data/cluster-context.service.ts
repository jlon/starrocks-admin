import { Injectable } from '@angular/core';
import { BehaviorSubject, Observable, of } from 'rxjs';
import { catchError, tap, distinctUntilChanged } from 'rxjs/operators';
import { Cluster, ClusterService } from './cluster.service';
import { PermissionService } from './permission.service';
import { AuthService } from './auth.service';

/**
 * Global cluster context service
 * Manages the currently active cluster across the application
 * Gets active cluster from backend instead of localStorage
 * 
 * ✅ Performance Best Practice:
 * - Frontend should check hasActiveCluster() BEFORE sending API requests
 * - This prevents unnecessary API calls to backend when no cluster is active
 * - Backend still validates cluster activation for security (fail-fast on 404)
 * 
 * ✅ Usage Pattern:
 * ```typescript
 * // In page components or services:
 * if (this.clusterContext.hasActiveCluster()) {
 *   this.loadData();  // Only send request if cluster is active
 * } else {
 *   this.toastrService.danger('请先激活一个集群');
 * }
 * ```
 */
@Injectable({
  providedIn: 'root',
})
export class ClusterContextService {
  // Current active cluster
  private activeClusterSubject: BehaviorSubject<Cluster | null>;
  public activeCluster$: Observable<Cluster | null>;
  private isRefreshing = false; // Flag to prevent concurrent refresh calls
  
  constructor(
    private clusterService: ClusterService,
    private permissionService: PermissionService,
    private authService: AuthService,
  ) {
    this.activeClusterSubject = new BehaviorSubject<Cluster | null>(null);
    this.activeCluster$ = this.activeClusterSubject.asObservable();
    
    // Try to load active cluster from backend on initialization (only if authenticated)
    if (this.authService.isAuthenticated()) {
      this.refreshActiveCluster();
    }

    // Listen to permission changes (triggers on login/logout)
    // Use distinctUntilChanged to avoid duplicate calls when permissions array reference changes but content is same
    this.permissionService.permissions$.pipe(
      distinctUntilChanged((prev, curr) => prev.length === curr.length && prev.every((p, i) => p.id === curr[i]?.id))
    ).subscribe(() => {
      // Only refresh if user is authenticated
      if (this.authService.isAuthenticated()) {
        this.refreshActiveCluster();
      } else {
        // Clear active cluster when user logs out
        this.clearActiveCluster();
      }
    });

    // Listen to user changes directly to catch logout immediately
    // Use distinctUntilChanged to avoid duplicate calls when user object reference changes but content is same
    this.authService.currentUser.pipe(
      distinctUntilChanged((prev, curr) => {
        // Compare by user ID, or both null
        if (!prev && !curr) return true;
        if (!prev || !curr) return false;
        return prev.id === curr.id;
      })
    ).subscribe((user) => {
      if (!user) {
        // User logged out, clear active cluster immediately
        this.clearActiveCluster();
      } else if (this.authService.isAuthenticated()) {
        // User logged in, refresh active cluster
        this.refreshActiveCluster();
      }
    });
  }
  
  /**
   * Set the active cluster by calling backend API
   */
  setActiveCluster(cluster: Cluster): void {
    if (!this.permissionService.hasPermission('api:clusters:activate')) {
      return;
    }
    // Call backend API to activate the cluster
    this.clusterService.activateCluster(cluster.id).pipe(
      tap((activatedCluster) => {
        this.activeClusterSubject.next(activatedCluster);
      }),
      catchError((error) => {
        // Still update local state for immediate feedback
        this.activeClusterSubject.next(cluster);
        return of(cluster);
      })
    ).subscribe();
  }
  
  /**
   * Refresh active cluster from backend
   * Note: We don't check permission here, let the backend decide.
   * This allows users with page access to see the active cluster name,
   * even if they don't have explicit 'api:clusters:active' permission.
   * The backend will return 403 if the user truly doesn't have permission.
   */
  refreshActiveCluster(): void {
    // Don't refresh if user is not authenticated (logged out)
    if (!this.authService.isAuthenticated()) {
      this.clearActiveCluster();
      return;
    }
    
    // Prevent concurrent refresh calls to avoid duplicate requests
    if (this.isRefreshing) {
      return;
    }
    
    this.isRefreshing = true;
    
    // Always try to get active cluster from backend
    // Backend will handle permission checking
    this.clusterService.getActiveCluster().pipe(
      tap((cluster) => {
        this.activeClusterSubject.next(cluster);
        this.isRefreshing = false;
      }),
      catchError((error) => {
        this.isRefreshing = false;
        // If backend returns 401 (unauthorized) or 403 (forbidden), clear active cluster
        // This is expected when user logs out or doesn't have permission
        if (error.status === 401 || error.status === 403) {
          this.clearActiveCluster();
        } else {
          // For other errors, keep current state
          this.activeClusterSubject.next(null);
        }
        return of(null);
      })
    ).subscribe();
  }
  
  /**
   * Get the current active cluster
   */
  getActiveCluster(): Cluster | null {
    return this.activeClusterSubject.value;
  }
  
  /**
   * Get the active cluster ID
   */
  getActiveClusterId(): number | null {
    const cluster = this.activeClusterSubject.value;
    return cluster ? cluster.id : null;
  }
  
  /**
   * Clear active cluster
   */
  clearActiveCluster(): void {
    this.activeClusterSubject.next(null);
  }
  
  /**
   * Check if a cluster is active (RECOMMENDED: Check before API calls)
   * This helps optimize performance by not sending requests when no cluster is active
   */
  hasActiveCluster(): boolean {
    return this.activeClusterSubject.value !== null;
  }
}


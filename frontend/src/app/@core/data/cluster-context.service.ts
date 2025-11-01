import { Injectable } from '@angular/core';
import { BehaviorSubject, Observable, of } from 'rxjs';
import { catchError, tap } from 'rxjs/operators';
import { Cluster, ClusterService } from './cluster.service';
import { PermissionService } from './permission.service';

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
  
  constructor(
    private clusterService: ClusterService,
    private permissionService: PermissionService,
  ) {
    this.activeClusterSubject = new BehaviorSubject<Cluster | null>(null);
    this.activeCluster$ = this.activeClusterSubject.asObservable();
    
    // Try to load active cluster from backend on initialization
    this.refreshActiveCluster();

    this.permissionService.permissions$.subscribe(() => {
      this.refreshActiveCluster();
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
   */
  refreshActiveCluster(): void {
    if (!this.permissionService.hasPermission('api:clusters:active')) {
      this.activeClusterSubject.next(null);
      return;
    }
    
    this.clusterService.getActiveCluster().pipe(
      tap((cluster) => {
        this.activeClusterSubject.next(cluster);
      }),
      catchError((error) => {
        this.activeClusterSubject.next(null);
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


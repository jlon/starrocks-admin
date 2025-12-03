import { Component, OnInit, OnDestroy, TemplateRef, ViewChild } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { HttpClient } from '@angular/common/http';
import { NbToastrService, NbDialogService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { NodeService } from '../../../../@core/data/node.service';
import { ClusterContextService } from '../../../../@core/data/cluster-context.service';
import { Cluster } from '../../../../@core/data/cluster.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { MetricThresholds, renderMetricBadge, parseStarRocksDuration } from '../../../../@core/utils/metric-badge';
import { renderLongText } from '../../../../@core/utils/text-truncate';
import { AuthService } from '../../../../@core/data/auth.service';
import * as dagre from 'dagre';

@Component({
  selector: 'ngx-profile-queries',
  templateUrl: './profile-queries.component.html',
  styleUrls: ['./profile-queries.component.scss'],
})
export class ProfileQueriesComponent implements OnInit, OnDestroy {
  // Data sources
  profileSource: LocalDataSource = new LocalDataSource();
  
  // State
  clusterId: number;
  activeCluster: Cluster | null = null;
  loading = true;
  autoRefresh = false; // Default: disabled
  refreshInterval: any;
  selectedRefreshInterval: number | 'off' = 'off'; // Default: off (Grafana style)
  refreshIntervalOptions = [
    { value: 'off', label: '关闭' },
    { value: 3, label: '3秒' },
    { value: 5, label: '5秒' },
    { value: 10, label: '10秒' },
    { value: 30, label: '30秒' },
    { value: 60, label: '1分钟' },
  ];
  private destroy$ = new Subject<void>();
  private profileDurationThresholds: MetricThresholds = { warn: 120000, danger: 240000 }; // Will be updated dynamically

  // Profile dialog
  currentProfileDetail: string = '';
  currentQueryId: string = '';
  profileDetailLoading = false;
  @ViewChild('profileDetailDialog') profileDetailDialogTemplate: TemplateRef<any>;
  
  // DAG Analysis
  analysisLoading = false;
  analysisError: string = '';
  analysisData: any = null;
  topNodes: any[] = [];
  graphNodes: any[] = [];
  graphEdges: any[] = [];
  graphWidth = 800;
  graphHeight = 600;
  selectedNode: any = null;

  // Profile management settings
  profileSettings = {
    mode: 'external',
    hideSubHeader: false, // Enable search
    noDataMessage: '暂无Profile记录',
    actions: {
      add: false,
      edit: true,
      delete: false,
      position: 'right',
      width: '80px',
    },
    edit: {
      editButtonContent: '<i class="nb-search"></i>',
    },
    pager: {
      display: true,
      perPage: 20,
    },
    columns: {
      QueryId: { title: 'Query ID', type: 'string', width: '25%' },
      StartTime: { title: '开始时间', type: 'string', width: '15%' },
      Time: {
        title: '执行时间',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string | number) => {
          // Parse StarRocks duration string to milliseconds for accurate threshold comparison
          const durationMs = parseStarRocksDuration(value);
          return renderMetricBadge(durationMs, this.profileDurationThresholds, {
            labelFormatter: (val) => {
              // Use original string for display, but parsed number for thresholds
              return typeof value === 'string' ? value : `${val}ms`;
            }
          });
        },
      },
      State: {
        title: '状态',
        type: 'html',
        width: '10%',
        valuePrepareFunction: (value: string) => {
          const status = value === 'Finished' ? 'success' : 'warning';
          return `<span class="badge badge-${status}">${value}</span>`;
        },
      },
      Statement: { 
        title: 'SQL语句', 
        type: 'html', 
        width: '40%',
        valuePrepareFunction: (value: any) => renderLongText(value, 100),
      },
    },
  };

  constructor(
    private route: ActivatedRoute,
    private http: HttpClient,
    private nodeService: NodeService,
    private clusterContextService: ClusterContextService,
    private toastrService: NbToastrService,
    private dialogService: NbDialogService,
    private authService: AuthService,
  ) {
    // Try to get clusterId from route first (for direct navigation)
    const routeClusterId = parseInt(this.route.snapshot.paramMap.get('clusterId') || '0', 10);
    this.clusterId = routeClusterId;
  }

  ngOnInit(): void {
    // Subscribe to active cluster changes
    this.clusterContextService.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
        if (cluster) {
          // Always use the active cluster (override route parameter)
          const newClusterId = cluster.id;
          if (this.clusterId !== newClusterId) {
            this.clusterId = newClusterId;
            this.loadProfiles();
          }
        }
        // Backend will handle "no active cluster" case
      });

    // Load data - backend will get active cluster automatically
    this.loadProfiles();
  }

  ngOnDestroy(): void {
    this.stopAutoRefresh();
    this.destroy$.next();
    this.destroy$.complete();
  }

  // Grafana-style: selecting an interval automatically enables auto-refresh
  // Selecting 'off' disables auto-refresh
  onRefreshIntervalChange(interval: number | 'off'): void {
    this.selectedRefreshInterval = interval;
    
    if (interval === 'off') {
      // Disable auto-refresh
      this.autoRefresh = false;
      this.stopAutoRefresh();
    } else {
      // Enable auto-refresh with selected interval
      this.autoRefresh = true;
      this.stopAutoRefresh();
      this.startAutoRefresh();
    }
  }

  startAutoRefresh(): void {
    this.stopAutoRefresh(); // Clear any existing interval
    
    // Only start if interval is a number (not 'off')
    if (typeof this.selectedRefreshInterval !== 'number') {
      return;
    }
    
    this.refreshInterval = setInterval(() => {
      // Stop auto-refresh if user is not authenticated (logged out)
      if (!this.authService.isAuthenticated()) {
        this.autoRefresh = false;
        this.selectedRefreshInterval = 'off';
        this.stopAutoRefresh();
        return;
      }
      // Only update data, don't show loading spinner during auto-refresh
      this.loadProfilesSilently();
    }, this.selectedRefreshInterval * 1000);
  }

  stopAutoRefresh(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  // Load profiles
  loadProfiles(): void {
    this.loading = true;
    this.nodeService.listProfiles().subscribe(
      data => {
        this.profileSource.load(data);
        this.updateDynamicThresholds(data);
        this.loading = false;
      },
      error => {
        this.toastrService.danger(ErrorHandler.handleClusterError(error), '加载失败');
        this.loading = false;
      }
    );
  }

  // Load profiles silently (for auto-refresh, without loading spinner)
  loadProfilesSilently(): void {
    this.nodeService.listProfiles().subscribe(
      data => {
        this.profileSource.load(data);
        this.updateDynamicThresholds(data);
      },
      error => {
        // Silently handle errors during auto-refresh
        console.error('Failed to refresh profiles:', error);
      }
    );
  }

  /**
   * Update dynamic thresholds based on maximum time in current data
   * Algorithm:
   * - Find the maximum execution time in the dataset
   * - Red (danger): > max_time * 70%
   * - Yellow (warning): > max_time * 40% and <= max_time * 70%
   * - Green (success): <= max_time * 40%
   * 
   * This ensures color coding adapts to the actual data range
   */
  updateDynamicThresholds(profiles: any[]): void {
    if (!profiles || profiles.length === 0) {
      return;
    }

    // Extract duration values from profiles
    const durationValues = profiles
      .map(profile => parseStarRocksDuration(profile.Time))
      .filter(value => !isNaN(value) && value > 0);

    if (durationValues.length === 0) {
      // No valid data, use defaults
      return;
    }

    // Find maximum time
    const maxTime = Math.max(...durationValues);
    
    // Calculate thresholds based on max time percentage
    // Red: > 70% of max time
    // Yellow: > 40% of max time
    const warnThreshold = maxTime * 0.5;   // 40% of max
    const dangerThreshold = maxTime * 0.8; // 70% of max

    // Update thresholds
    this.profileDurationThresholds = {
      warn: warnThreshold,
      danger: dangerThreshold,
    };
  }

  // Helper: Format milliseconds to readable duration
  private formatDuration(ms: number): string {
    if (ms < 1000) return `${ms}ms`;
    if (ms < 60000) return `${(ms / 1000).toFixed(2)}s`;
    return `${(ms / 60000).toFixed(2)}m`;
  }

  // Handle profile edit action (view profile)
  onProfileEdit(event: any): void {
    this.viewProfileDetail(event.data.QueryId);
  }

  // View profile detail from profile list
  viewProfileDetail(queryId: string): void {
    this.currentQueryId = queryId;
    this.profileDetailLoading = true;
    this.analysisLoading = true;
    this.currentProfileDetail = '';
    this.analysisData = null;
    this.analysisError = '';
    this.topNodes = [];
    this.graphNodes = [];
    this.graphEdges = [];
    this.selectedNode = null;
    
    // Open dialog first with loading state
    this.dialogService.open(this.profileDetailDialogTemplate, {
      context: { profile: this.currentProfileDetail },
      hasBackdrop: true,
      closeOnBackdropClick: true,
      closeOnEsc: true,
      dialogClass: 'profile-dialog-lg',
    });
    
    // Load profile text
    this.nodeService.getProfile(queryId).subscribe(
      data => {
        this.currentProfileDetail = data.profile_content;
        this.profileDetailLoading = false;
      },
      error => {
        this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '加载失败');
        this.profileDetailLoading = false;
      }
    );
    
    // Load analysis data
    this.loadAnalysis(queryId);
  }
  
  // Load profile analysis for DAG
  loadAnalysis(queryId: string): void {
    this.analysisLoading = true;
    this.analysisError = '';
    
    this.http.get<any>(`/api/clusters/profiles/${queryId}/analyze`).subscribe({
      next: (data) => {
        this.analysisData = data;
        this.topNodes = data.summary?.top_time_consuming_nodes || [];
        if (data.execution_tree) {
          this.buildGraph(data.execution_tree);
        }
        this.analysisLoading = false;
      },
      error: (err) => {
        console.error('Failed to analyze profile', err);
        this.analysisError = '分析失败: ' + (err.error?.message || err.message || '未知错误');
        this.analysisLoading = false;
      }
    });
  }
  
  // Refresh analysis
  refreshAnalysis(): void {
    if (this.currentQueryId) {
      this.loadAnalysis(this.currentQueryId);
    }
  }
  
  // Build DAG graph using dagre
  buildGraph(tree: any): void {
    const g = new dagre.graphlib.Graph();
    g.setGraph({ 
      rankdir: 'BT',
      marginx: 20, 
      marginy: 20,
      nodesep: 50,
      ranksep: 80
    });
    g.setDefaultEdgeLabel(() => ({}));

    const nodeList = tree.nodes || [];

    // Add nodes
    nodeList.forEach((node: any) => {
      g.setNode(node.id, { width: 200, height: 90 });
    });

    // Add edges
    nodeList.forEach((node: any) => {
      if (node.children) {
        node.children.forEach((childId: string) => {
          g.setEdge(childId, node.id);
        });
      }
    });

    // Calculate layout
    dagre.layout(g);

    // Extract coordinates
    this.graphNodes = nodeList.map((node: any) => {
      const layoutNode = g.node(node.id);
      return {
        ...node,
        x: layoutNode.x,
        y: layoutNode.y,
        width: layoutNode.width,
        height: layoutNode.height
      };
    });

    this.graphEdges = g.edges().map((e: any) => {
      const edge = g.edge(e);
      return {
        v: e.v,
        w: e.w,
        points: edge.points
      };
    });

    // Calculate bounding box
    const maxX = Math.max(...this.graphNodes.map((n: any) => (n.x || 0) + (n.width || 0)/2));
    const maxY = Math.max(...this.graphNodes.map((n: any) => (n.y || 0) + (n.height || 0)/2));
    this.graphWidth = Math.max(maxX + 50, 600);
    this.graphHeight = Math.max(maxY + 50, 400);
  }
  
  // Select a node
  selectNode(node: any): void {
    this.selectedNode = node;
  }
  
  // Get edge path for SVG
  getEdgePath(points: {x: number, y: number}[]): string {
    if (!points || points.length === 0) return '';
    return 'M' + points.map(p => `${p.x},${p.y}`).join(' L');
  }
  
  // Get percentage class for styling
  getPercentageClass(pct: number): string {
    if (!pct) return 'normal';
    if (pct > 30) return 'danger';
    if (pct > 15) return 'warning';
    return 'normal';
  }
  
  // Format duration in nanoseconds
  formatDurationNs(ns: any): string {
    if (!ns) return '-';
    const val = Number(ns);
    if (isNaN(val)) return ns;
    
    if (val < 1000) return val + 'ns';
    if (val < 1000000) return (val/1000).toFixed(2) + 'us';
    if (val < 1000000000) return (val/1000000).toFixed(2) + 'ms';
    return (val/1000000000).toFixed(2) + 's';
  }

  // Copy profile content to clipboard
  copyProfileToClipboard(): void {
    if (!this.currentProfileDetail) {
      return;
    }

    // Use Clipboard API
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(this.currentProfileDetail)
        .then(() => {
          this.toastrService.success('Profile 内容已复制到剪贴板', '复制成功');
        })
        .catch(err => {
          console.error('Failed to copy:', err);
          this.fallbackCopy();
        });
    } else {
      // Fallback for older browsers or non-secure contexts
      this.fallbackCopy();
    }
  }

  // Fallback copy method for older browsers
  private fallbackCopy(): void {
    const textArea = document.createElement('textarea');
    textArea.value = this.currentProfileDetail;
    textArea.style.position = 'fixed';
    textArea.style.left = '-999999px';
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();

    try {
      const successful = document.execCommand('copy');
      if (successful) {
        this.toastrService.success('Profile 内容已复制到剪贴板', '复制成功');
      } else {
        this.toastrService.warning('复制失败，请手动复制', '提示');
      }
    } catch (err) {
      console.error('Failed to copy:', err);
      this.toastrService.warning('复制失败，请手动复制', '提示');
    } finally {
      document.body.removeChild(textArea);
    }
  }
}

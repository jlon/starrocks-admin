import { ChangeDetectorRef, Component, OnInit, OnDestroy, TemplateRef, ViewChild, ViewEncapsulation } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { Location } from '@angular/common'; // Import Location
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
  encapsulation: ViewEncapsulation.None,
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
  zoomLevel = 1; // Zoom level for DAG
  // Panning state
  isPanning = false;
  startX = 0;
  startY = 0;
  translateX = 0;
  translateY = 0;
  
  // Window control state
  isFullscreen = false; // Default to normal layout, toggle for full screen
  
  // Right Panel State (default width expanded by 30%)
  rightPanelWidth = 500;
  isRightPanelCollapsed = false;
  isResizingRight = false;
  
  // Right panel section collapse states (Figure 1 style)
  isTop10Collapsed = false;
  isSummaryCollapsed = false;
  isDiagnosisCollapsed = false;
  showMemoryView = false; // Toggle between Time and Memory view
  
  private nodeRankMap: Map<string, number> = new Map(); // Node rank by time percentage
  objectKeys = Object.keys; // Helper for template

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
    private nodeService: NodeService,
    private clusterContextService: ClusterContextService,
    private toastrService: NbToastrService,
    private dialogService: NbDialogService,
    private authService: AuthService,
    private cdr: ChangeDetectorRef,
    private location: Location, // Inject Location
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
    
    this.nodeService.analyzeProfile(queryId).subscribe({
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
  
  // DEBUG: Test method with MOCK DATA to guarantee display
  testSpecificQuery(): void {
    console.log('Testing with MOCK DATA');
    
    // Construct mock tree structure matching backend format
    const mockTree = {
      root: { 
        id: 'node_-1', 
        operator_name: 'RESULT_SINK', 
        plan_node_id: -1, 
        children: ['node_0'], 
        metrics: { operator_total_time: 100000 }, 
        time_percentage: 0.5,
        rows: 1
      },
      nodes: [
        { id: 'node_0', operator_name: 'HASH_JOIN', plan_node_id: 0, children: ['node_1', 'node_2'], metrics: { operator_total_time: 5000000 }, time_percentage: 15.5, rows: 1 },
        { id: 'node_1', operator_name: 'OLAP_SCAN', plan_node_id: 1, children: [], metrics: { operator_total_time: 12000000 }, time_percentage: 35.2, rows: 1000 },
        { id: 'node_2', operator_name: 'EXCHANGE', plan_node_id: 2, children: ['node_3'], metrics: { operator_total_time: 2000000 }, time_percentage: 5.1, rows: 1 },
        { id: 'node_3', operator_name: 'AGGREGATION', plan_node_id: 3, children: ['node_4', 'node_5'], metrics: { operator_total_time: 8000000 }, time_percentage: 22.3, rows: 500 },
        { id: 'node_4', operator_name: 'OLAP_SCAN', plan_node_id: 4, children: [], metrics: { operator_total_time: 4000000 }, time_percentage: 12.1, rows: 2000 },
        { id: 'node_5', operator_name: 'PROJECT', plan_node_id: 5, children: ['node_6'], metrics: { operator_total_time: 500000 }, time_percentage: 1.2, rows: 100 },
        { id: 'node_6', operator_name: 'OLAP_SCAN', plan_node_id: 6, children: [], metrics: { operator_total_time: 3000000 }, time_percentage: 8.1, rows: 3000 }
      ]
    };
    
    // Set analysis data directly
    this.analysisData = {
      execution_tree: mockTree,
      summary: { query_id: 'MOCK-TEST', top_time_consuming_nodes: [] },
      hotspots: [],
      conclusion: 'Mock Analysis - Testing DAG Display',
      suggestions: [],
      performance_score: 85
    };
    
    // Open dialog first
    this.dialogService.open(this.profileDetailDialogTemplate, {
      context: { QueryId: 'MOCK-TEST' },
      dialogClass: 'profile-detail-dialog',
      closeOnBackdropClick: false,
    });
    
    // Build graph after dialog opens
    setTimeout(() => {
      this.buildGraph(mockTree);
      this.cdr.markForCheck();
    }, 100);
  }

  private getIntersectionPoint(
    from: { x: number, y: number },
    target: { x: number, y: number, width: number, height: number }
  ): { x: number, y: number } {
    const dx = from.x - target.x;
    const dy = from.y - target.y;
    if (dx === 0 && dy === 0) {
      return { ...target };
    }

    const halfW = target.width / 2;
    const halfH = target.height / 2;
    const tx = dx !== 0 ? halfW / Math.abs(dx) : Infinity;
    const ty = dy !== 0 ? halfH / Math.abs(dy) : Infinity;
    const t = Math.min(tx, ty);

    return {
      x: target.x + t * dx,
      y: target.y + t * dy,
    };
  }

  // Build DAG graph using dagre
  buildGraph(tree: any): void {
    const g = new dagre.graphlib.Graph();
    g.setGraph({ 
      rankdir: 'BT',
      marginx: 40, 
      marginy: 40,
      nodesep: 80,  // Increase node separation
      ranksep: 100  // Increase rank separation
    });
    g.setDefaultEdgeLabel(() => ({}));

    // Create a shallow copy of nodes array to avoid mutating the original data
    const nodeList = [...(tree.nodes || [])];
    if (tree.root) {
      // Check if root is already in the list to avoid duplicates
      if (!nodeList.find((n: any) => n.id === tree.root.id)) {
        nodeList.unshift(tree.root);
      }
    }

    console.log('Building graph with nodes:', nodeList.length);

    // Add nodes
    nodeList.forEach((node: any) => {
      // Node height: Header(35) + Body(55) + Progress(3) ≈ 93px
      // Use 95px for dagre layout to ensure proper spacing
      g.setNode(node.id, { width: 220, height: 95 });
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

    console.log('Graph layout complete:', {
      width: g.graph().width,
      height: g.graph().height,
      nodes: g.nodes().length,
      edges: g.edges().length
    });

    // Extract coordinates
    this.graphNodes = nodeList.map((node: any) => {
      const layoutNode = g.node(node.id);
      
      // Calculate coordinates for sanitized output
      const x = (layoutNode && isFinite(layoutNode.x)) ? layoutNode.x : 0;
      const y = (layoutNode && isFinite(layoutNode.y)) ? layoutNode.y : 0;
      const width = (layoutNode && isFinite(layoutNode.width)) ? layoutNode.width : 220;
      const height = (layoutNode && isFinite(layoutNode.height)) ? layoutNode.height : 95;

      return {
        ...node,
        x,
        y,
        width,
        height
      };
    });

    // First pass: collect all rows values to determine max for stroke width calculation
    const allRows = nodeList.map((n: any) => n.rows || 0).filter((r: number) => r > 0);
    const maxRows = allRows.length > 0 ? Math.max(...allRows) : 1;

    this.graphEdges = g.edges().map((e: any, index: number) => {
      const edge = g.edge(e);
      const sourceNodeData = nodeList.find((n: any) => n.id === e.v);
      const sourceNode = this.graphNodes.find((n: any) => n.id === e.v);
      const targetNode = this.graphNodes.find((n: any) => n.id === e.w);
      const rows = sourceNodeData?.rows || 0;

      let labelFormatted = rows > 0 ? 'Rows: ' + Number(rows).toLocaleString() : '';

      if (targetNode && (
          targetNode.operator_name.includes('JOIN') ||
          targetNode.operator_name === 'NESTLOOP_JOIN' ||
          targetNode.operator_name === 'HASH_JOIN'
        ) && targetNode.children && targetNode.children.length >= 2) {
        if (targetNode.children[0] === e.v) {
          labelFormatted += ' (PROBE)';
        } else if (targetNode.children[1] === e.v) {
          labelFormatted += ' (BUILD)';
        }
      }

      // Calculate dynamic stroke width based on rows (Algorithm: Logarithmic Scale)
      // Use Log scale because rows can vary from 0 to billions. Linear scale makes small diffs invisible.
      const minStrokeWidth = 1.5;
      const maxStrokeWidth = 6;
      
      let strokeWidth = minStrokeWidth;
      if (rows > 0 && maxRows > 0) {
        const logRows = Math.log10(rows + 1); // +1 to avoid log(0)
        const logMax = Math.log10(maxRows + 1);
        // Calculate ratio based on orders of magnitude
        const ratio = logMax > 0 ? logRows / logMax : 0;
        strokeWidth = minStrokeWidth + ratio * (maxStrokeWidth - minStrokeWidth);
      }

      // Calculate target position for multi-child nodes (Algorithm: Center Distribution)
      // Instead of spreading across the full width, distribute from center with fixed spacing
      // This looks better and more "connected"
      let targetX = targetNode?.x || 0;
      if (targetNode && targetNode.children && targetNode.children.length > 1) {
        const childIndex = targetNode.children.indexOf(e.v);
        const numChildren = targetNode.children.length;
        const spacing = 40; // 40px spacing between connection points
        
        // Calculate offset from center
        // e.g. 2 children: -20, +20
        // e.g. 3 children: -40, 0, +40
        const centerOffset = (childIndex - (numChildren - 1) / 2) * spacing;
        targetX = targetNode.x + centerOffset;
      }

      // Calculate visible line segment
      let visibleStart = { x: 0, y: 0 };
      let visibleEnd = { x: 0, y: 0 };
      
      if (sourceNode && targetNode) {
        // Source: arrow starts from node's TOP edge
        visibleStart = {
          x: sourceNode.x,
          y: sourceNode.y - sourceNode.height / 2
        };
        // Target: arrow ends at node's BOTTOM edge
        // Use dagre height/2 as base, will be fine-tuned by updateEdgesAfterRender()
        visibleEnd = {
          x: targetX,
          y: targetNode.y + targetNode.height / 2
        };
      }

      // Define points for the line
      const displayPoints = [visibleStart, visibleEnd];

      // Label position: geometric middle of visible segment
      const labelPos = {
        x: (visibleStart.x + visibleEnd.x) / 2,
        y: (visibleStart.y + visibleEnd.y) / 2
      };

      // Determine stroke color based on target node type
      let strokeColor = '#bfbfbf';
      if (targetNode) {
        const name = targetNode.operator_name?.toUpperCase() || '';
        if (name.includes('SCAN') || name.includes('JOIN')) {
          strokeColor = '#fa8c16';
        }
      }

      // Calculate arrow size based on stroke width
      const arrowSize = Math.max(8, strokeWidth * 2);

      return {
        v: e.v,
        w: e.w,
        points: displayPoints,
        markerId: `edge-marker-${e.v}-${e.w}-${index}`,
        labelPos,
        strokeColor,
        strokeWidth,
        arrowSize,
        label: rows,
        labelFormatted,
      };
    });
    
    // Calculate bounding box
    const maxX = Math.max(...this.graphNodes.map((n: any) => (n.x || 0) + (n.width || 0)/2));
    const maxY = Math.max(...this.graphNodes.map((n: any) => (n.y || 0) + (n.height || 0)/2));
    this.graphWidth = Math.max(maxX + 50, 600);
    this.graphHeight = Math.max(maxY + 50, 400);
    
    // Force change detection to update view
    this.cdr.markForCheck();
    
    // DEBUG: Log final results
    console.log('=== 构建完成 ===');
    console.log('graphNodes 数量:', this.graphNodes.length);
    console.log('graphEdges 数量:', this.graphEdges.length);
    console.log('图表尺寸:', this.graphWidth, 'x', this.graphHeight);
    if (this.graphNodes.length > 0) {
      console.log('第一个节点:', {
        id: this.graphNodes[0].id,
        operator_name: this.graphNodes[0].operator_name,
        x: this.graphNodes[0].x,
        y: this.graphNodes[0].y,
        width: this.graphNodes[0].width,
        height: this.graphNodes[0].height
      });
    }
    console.log('=============');
    
    // Calculate node ranks for color coding
    this.calculateNodeRanks();
    
    // Center the graph after layout, then update edges based on actual DOM heights
    setTimeout(() => {
      this.centerGraph();
      this.updateEdgesAfterRender();
    });
  }
  
  // Update edge positions after DOM has rendered (measure actual node heights)
  private updateEdgesAfterRender(): void {
    const nodeElements = document.querySelectorAll('.dag-node');
    if (!nodeElements.length) return;
    
    // Build a map of actual DOM heights
    const actualHeights: Map<string, number> = new Map();
    nodeElements.forEach((el: Element) => {
      const nodeId = el.getAttribute('data-node-id');
      if (nodeId) {
        actualHeights.set(nodeId, el.getBoundingClientRect().height / this.zoomLevel);
      }
    });
    
    // Update graphNodes with actual heights
    this.graphNodes.forEach(node => {
      const actualH = actualHeights.get(node.id);
      if (actualH && actualH > 0) {
        node.actualHeight = actualH;
      }
    });
    
    // Recalculate edge endpoints using actual heights
    this.graphEdges = this.graphEdges.map(edge => {
      const sourceNode = this.graphNodes.find(n => n.id === edge.v);
      const targetNode = this.graphNodes.find(n => n.id === edge.w);
      
      if (sourceNode && targetNode) {
        const sourceActualH = sourceNode.actualHeight || sourceNode.height;
        const targetActualH = targetNode.actualHeight || targetNode.height;
        
        // DOM node is positioned at: top = node.y - dagreHeight/2
        // So actual TOP edge = node.y - dagreHeight/2
        // And actual BOTTOM edge = (node.y - dagreHeight/2) + actualDOMHeight
        
        // Source node: arrow starts from its TOP edge
        const newStartY = sourceNode.y - sourceNode.height / 2;
        // Target node: arrow ends at its BOTTOM edge
        // BOTTOM = (node.y - dagreHeight/2) + actualDOMHeight
        const newEndY = (targetNode.y - targetNode.height / 2) + targetActualH;
        
        // Update points
        if (edge.points && edge.points.length >= 2) {
          edge.points[0] = { x: edge.points[0].x, y: newStartY };
          edge.points[1] = { x: edge.points[1].x, y: newEndY };
        }
        
        // Update label position (middle of line)
        edge.labelPos = {
          x: (edge.points[0].x + edge.points[1].x) / 2,
          y: (newStartY + newEndY) / 2
        };
      }
      
      return edge;
    });
    console.log('Edges updated with actual DOM heights');
    this.cdr.markForCheck();
  }
  
  // Center the graph in the viewport
  centerGraph(): void {
    const viewport = document.querySelector('.dag-center-panel') as HTMLElement;
    if (viewport && this.graphWidth > 0 && this.graphHeight > 0) {
      const vw = viewport.clientWidth;
      const vh = viewport.clientHeight;
      // Calculate center
      this.translateX = (vw - this.graphWidth * this.zoomLevel) / 2;
      this.translateY = (vh - this.graphHeight * this.zoomLevel) / 2;
      // Ensure some padding top if it's too high
      if (this.translateY < 20) this.translateY = 20;
      if (this.translateX < 20) this.translateX = 20;
    }
  }
  
  // Select a node
  selectNode(node: any, event?: Event): void {
    if (event) {
      event.stopPropagation(); // Prevent panel click from clearing selection
    }
    this.selectedNode = node;
  }
  
  // Handle click on DAG panel background (clear selection)
  onDagPanelClick(event: Event): void {
    // Only clear if clicking on the panel itself, not on nodes
    const target = event.target as HTMLElement;
    if (!target.closest('.dag-node') && !target.closest('.dag-toolbar')) {
      this.clearSelectedNode();
    }
  }
  
  // Select node by ID (for top10 table click)
  selectNodeById(nodeId: string): void {
    const node = this.graphNodes.find(n => n.id === nodeId);
    if (node) {
      this.selectedNode = node;
    }
  }
  
  // Clear selected node (return to summary view)
  clearSelectedNode(): void {
    this.selectedNode = null;
  }
  
  // Get top 10 nodes by time percentage (include all nodes, even 0%)
  get top10NodesByTime(): any[] {
    if (!this.graphNodes || this.graphNodes.length === 0) return [];
    return [...this.graphNodes]
      .filter(n => n.time_percentage !== undefined && n.time_percentage !== null)
      .sort((a, b) => (b.time_percentage || 0) - (a.time_percentage || 0))
      .slice(0, 10);
  }
  
  // Get top 10 nodes by memory usage (show all nodes, even with null/0 memory)
  get top10NodesByMemory(): any[] {
    if (!this.graphNodes || this.graphNodes.length === 0) return [];
    return [...this.graphNodes]
      .sort((a, b) => (b.metrics?.memory_usage || 0) - (a.metrics?.memory_usage || 0))
      .slice(0, 10);
  }
  
  // Calculate scan time percentage
  getScanTimePercentage(): number {
    const scanMs = this.analysisData?.summary?.query_cumulative_scan_time_ms || 0;
    const totalMs = this.analysisData?.summary?.query_cumulative_operator_time_ms || 
                    this.analysisData?.summary?.query_execution_wall_time_ms || 1;
    return totalMs > 0 ? Math.min((scanMs / totalMs) * 100, 100) : 0;
  }
  
  // Calculate CPU time percentage
  getCpuTimePercentage(): number {
    const cpuMs = this.analysisData?.summary?.query_cumulative_cpu_time_ms || 0;
    const totalMs = this.analysisData?.summary?.query_cumulative_operator_time_ms || 
                    this.analysisData?.summary?.query_execution_wall_time_ms || 1;
    return totalMs > 0 ? Math.min((cpuMs / totalMs) * 100, 100) : 0;
  }
  
  // Format time string with percentage
  formatTimeWithPercent(timeStr: string | undefined, percent: number): string {
    if (!timeStr) return '-';
    return `${percent.toFixed(2)}%(${timeStr})`;
  }
  
  // Check if any summary metric is available
  hasAnyMetric(): boolean {
    const s = this.analysisData?.summary;
    if (!s) return false;
    return !!(s.query_execution_wall_time || s.query_cumulative_operator_time ||
              s.query_cumulative_cpu_time || s.query_cumulative_scan_time ||
              s.query_cumulative_network_time || s.query_peak_schedule_time ||
              s.result_deliver_time || s.query_sum_memory_usage || s.query_spill_bytes);
  }
  
  // Get metric keys from metrics object (filter out null/undefined and specialized)
  getMetricKeys(metrics: any): string[] {
    if (!metrics) return [];
    return Object.keys(metrics).filter(key => {
      const val = metrics[key];
      return val !== null && val !== undefined && key !== 'specialized';
    });
  }
  
  // Format metric value based on key name
  formatMetricValue(key: string, value: any): string {
    if (value === null || value === undefined) return '-';
    const lowerKey = key.toLowerCase();
    // Time metrics (in nanoseconds)
    if (lowerKey.includes('time')) {
      return this.formatDurationNs(value);
    }
    // Memory/bytes metrics
    if (lowerKey.includes('memory') || lowerKey.includes('bytes')) {
      return this.formatBytes(value);
    }
    // Number metrics
    if (typeof value === 'number') {
      return value.toLocaleString();
    }
    return String(value);
  }
  
  // Format bytes to human readable
  formatBytes(bytes: any): string {
    if (bytes === null || bytes === undefined) return '0 B';
    const val = Number(bytes);
    if (isNaN(val)) return String(bytes);
    if (val < 1024) return val + ' B';
    if (val < 1024 * 1024) return (val / 1024).toFixed(2) + ' KB';
    if (val < 1024 * 1024 * 1024) return (val / (1024 * 1024)).toFixed(2) + ' MB';
    return (val / (1024 * 1024 * 1024)).toFixed(2) + ' GB';
  }
  
  // Get edge path for SVG
  getEdgePath(points: {x: number, y: number}[]): string {
    if (!points || points.length === 0) return '';
    return 'M' + points.map(p => `${p.x},${p.y}`).join(' L');
  }

  // Get edge label position (geometric middle of the path)
  getEdgeLabelPosition(points: {x: number, y: number}[]): {x: number, y: number} {
    if (!points || points.length < 2) return { x: 0, y: 0 };
    
    // Calculate total length
    let totalLength = 0;
    const segments = [];
    for (let i = 0; i < points.length - 1; i++) {
      const dx = points[i+1].x - points[i].x;
      const dy = points[i+1].y - points[i].y;
      const dist = Math.sqrt(dx*dx + dy*dy);
      segments.push({ dist, p1: points[i], p2: points[i+1] });
      totalLength += dist;
    }
    
    let targetLen = totalLength / 2;
    let currentLen = 0;
    
    for (const seg of segments) {
      if (currentLen + seg.dist >= targetLen) {
        // Found the segment
        const remaining = targetLen - currentLen;
        const ratio = remaining / seg.dist;
        return {
          x: seg.p1.x + (seg.p2.x - seg.p1.x) * ratio,
          y: seg.p1.y + (seg.p2.y - seg.p1.y) * ratio,
        };
      }
      currentLen += seg.dist;
    }
    
    // Fallback
    const mid = Math.floor(points.length / 2);
    return { x: points[mid].x, y: points[mid].y };
  }

  getLabelTransform(points: {x: number, y: number}[]): string {
    const pos = this.getEdgeLabelPosition(points);
    return `translate(${pos.x}, ${pos.y})`;
  }

  // Zoom controls
  zoomIn(): void {
    this.zoomLevel = Math.min(this.zoomLevel + 0.1, 3);
  }

  zoomOut(): void {
    this.zoomLevel = Math.max(this.zoomLevel - 0.1, 0.2);
  }

  resetZoom(): void {
    this.zoomLevel = 1;
    this.translateX = 0;
    this.translateY = 0;
  }
  
  toggleFullscreen(): void {
    this.isFullscreen = !this.isFullscreen;
    setTimeout(() => {
        // Trigger resize event to re-layout if needed
        window.dispatchEvent(new Event('resize'));
    });
  }

  // Panning
  startPan(event: MouseEvent): void {
    // Prevent pan if resizing right panel
    if (this.isResizingRight) return;
    
    // Check if click target is a node or button
    const target = event.target as HTMLElement;
    if (target.closest('.dag-node') || target.closest('button')) return;

    if (event.button === 0) {
      this.isPanning = true;
      this.startX = event.clientX - this.translateX;
      this.startY = event.clientY - this.translateY;
      const viewport = document.querySelector('.dag-center-panel') as HTMLElement; // Use panel cursor
      if (viewport) viewport.style.cursor = 'grabbing';
    }
  }
  
  pan(event: MouseEvent): void {
    if (this.isPanning) {
      event.preventDefault();
      this.translateX = event.clientX - this.startX;
      this.translateY = event.clientY - this.startY;
    }
    // Note: resize handling is now done in startResizeRight() with dedicated listeners
  }
  
  endPan(): void {
    this.isPanning = false;
    this.isResizingRight = false;
    const viewport = document.querySelector('.dag-center-panel') as HTMLElement;
    if (viewport) {
        viewport.style.cursor = 'default';
    }
    document.body.style.cursor = 'default';
  }
  
  // Right Panel Resizing & Toggle
  toggleRightPanel(): void {
    this.isRightPanelCollapsed = !this.isRightPanelCollapsed;
  }
  
  startResizeRight(event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();
    this.isResizingRight = true;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none'; // 防止选择文本
    
    // 记录起始位置和宽度
    const startX = event.clientX;
    const startWidth = this.rightPanelWidth;
    
    // Add global event listeners for reliable resize tracking
    const onMouseMove = (e: MouseEvent) => {
      if (this.isResizingRight) {
        e.preventDefault();
        e.stopPropagation();
        
        // 使用拖动距离来计算新宽度（更可靠）
        const deltaX = startX - e.clientX;
        const newWidth = startWidth + deltaX;
        
        if (newWidth > 200 && newWidth < 800) {
          this.rightPanelWidth = newWidth;
        }
      }
    };
    
    const onMouseUp = (e: MouseEvent) => {
      e.preventDefault();
      this.isResizingRight = false;
      document.body.style.cursor = 'default';
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', onMouseMove, true);
      document.removeEventListener('mouseup', onMouseUp, true);
    };
    
    // 使用 capture 模式确保优先处理
    document.addEventListener('mousemove', onMouseMove, true);
    document.addEventListener('mouseup', onMouseUp, true);
  }
  
  // Mouse wheel zoom
  onWheel(event: WheelEvent): void {
    event.preventDefault();
    if (event.deltaY < 0) {
      this.zoomIn();
    } else {
      this.zoomOut();
    }
  }
  
  // Get node rank (1-3 for top 3 time-consuming nodes)
  getNodeRank(node: any): number {
    return this.nodeRankMap.get(node.id) || 0;
  }
  
  // Calculate node ranks based on time percentage
  private calculateNodeRanks(): void {
    this.nodeRankMap.clear();
    if (!this.graphNodes || this.graphNodes.length === 0) return;
    
    // Sort nodes by time_percentage descending
    const sorted = [...this.graphNodes]
      .filter(n => n.time_percentage > 0)
      .sort((a, b) => b.time_percentage - a.time_percentage);
    
    // Assign ranks to top 3
    for (let i = 0; i < Math.min(3, sorted.length); i++) {
      this.nodeRankMap.set(sorted[i].id, i + 1);
    }
  }
  
  // Format duration in nanoseconds
  formatDurationNs(ns: any): string {
    if (!ns) return '-';
    const val = Number(ns);
    if (isNaN(val)) return ns;
    
    if (val < 1000) return val + 'ns';
    if (val < 1000000) return (val/1000).toFixed(2) + 'us';
    if (val < 1000000000) return (val/1000000).toFixed(2) + 'ms';
    
    // Convert to seconds
    const totalSeconds = val / 1000000000;
    if (totalSeconds < 60) return totalSeconds.toFixed(2) + 's';
    
    // Convert to human-readable format for larger durations
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = Math.floor(totalSeconds % 60);
    const millis = Math.round((totalSeconds % 1) * 1000);
    
    let result = '';
    if (hours > 0) result += hours + 'h';
    if (minutes > 0) result += minutes + 'm';
    if (seconds > 0 || (hours === 0 && minutes === 0)) result += seconds + 's';
    if (millis > 0) result += millis + 'ms';
    
    return result || '0s';
  }

  // Fix for Angular base href issue with SVG markers
  get arrowMarkerUrl(): string {
    return `url(${this.location.path()}#dag-arrow)`;
  }

  // Get node header class based on operator type (Figure 1 style)
  getNodeHeaderClass(node: any): string {
    const name = node.operator_name?.toUpperCase() || '';
    if (this.getNodeRank(node) === 1) return 'header-red';
    if (name.includes('SCAN')) return 'header-orange';
    if (name.includes('JOIN')) return 'header-orange';
    if (name.includes('EXCHANGE')) return 'header-gray';
    if (name.includes('PROJECT')) return 'header-gray';
    if (name.includes('AGGREGATION')) return 'header-gray';
    return 'header-gray';
  }

  // Get progress bar color
  getProgressColor(node: any): string {
    const name = node.operator_name?.toUpperCase() || '';
    if (name.includes('SCAN')) return '#fa8c16';
    if (name.includes('JOIN')) return '#fa8c16';
    return '#d9d9d9';
  }

  // Toggle functions for right panel sections
  toggleTop10(): void { this.isTop10Collapsed = !this.isTop10Collapsed; }
  toggleSummary(): void { this.isSummaryCollapsed = !this.isSummaryCollapsed; }
  toggleDiagnosis(): void { this.isDiagnosisCollapsed = !this.isDiagnosisCollapsed; }
  toggleMemoryView(): void { 
    this.showMemoryView = !this.showMemoryView; 
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

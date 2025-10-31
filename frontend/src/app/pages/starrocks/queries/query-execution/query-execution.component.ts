import { Component, OnInit, OnDestroy, ViewChild, AfterViewInit, ElementRef, HostListener } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { NbToastrService, NbThemeService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { NodeService, QueryExecuteResult, SingleQueryResult } from '../../../../@core/data/node.service';
import { ClusterContextService } from '../../../../@core/data/cluster-context.service';
import { Cluster } from '../../../../@core/data/cluster.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { EditorView, basicSetup } from 'codemirror';
import { sql } from '@codemirror/lang-sql';
import { oneDark } from '@codemirror/theme-one-dark';
import { format } from 'sql-formatter';
import { trigger, transition, style, animate, state } from '@angular/animations';

type NavNodeType = 'catalog' | 'database' | 'group' | 'table';

interface NavTreeNode {
  id: string;
  name: string;
  type: NavNodeType;
  icon?: string;
  expanded?: boolean;
  loading?: boolean;
  children: NavTreeNode[];
  data?: {
    catalog?: string;
    database?: string;
    table?: string;
    originalName?: string;
    tablesLoaded?: boolean;
    tableCount?: number;
  };
}

@Component({
  selector: 'ngx-query-execution',
  templateUrl: './query-execution.component.html',
  styleUrls: ['./query-execution.component.scss'],
  animations: [
    trigger('editorCollapse', [
      state('expanded', style({ height: '*', opacity: 1, overflow: 'hidden' })),
      state('collapsed', style({ height: '0px', opacity: 0, paddingTop: 0, paddingBottom: 0, marginBottom: 0, overflow: 'hidden' })),
      transition('expanded <=> collapsed', animate('200ms ease')),
    ]),
  ],
})
export class QueryExecutionComponent implements OnInit, OnDestroy, AfterViewInit {
  @ViewChild('editorContainer', { static: false }) editorContainer!: ElementRef;
  // Template tabset reference removed as it's no longer required

  // Data sources
  runningSource: LocalDataSource = new LocalDataSource();
  
  // Expose Math to template
  Math = Math;
  
  // State
  clusterId: number;
  activeCluster: Cluster | null = null;
  loading = true;
  selectedTab = 'realtime'; // 'realtime' or 'running'
  autoRefresh = false; // Default: disabled
  refreshInterval: any;
  selectedRefreshInterval = 5; // Default 5 seconds
  refreshIntervalOptions = [
    { value: 3, label: '3秒' },
    { value: 5, label: '5秒' },
    { value: 10, label: '10秒' },
    { value: 30, label: '30秒' },
    { value: 60, label: '1分钟' },
  ];
  private destroy$ = new Subject<void>();

  // CodeMirror editor
  private editorView: EditorView | null = null;
  private currentTheme: string = 'default';

  // Catalog and Database selection
  catalogs: string[] = [];
  selectedCatalog: string = '';
  loadingCatalogs: boolean = false;
  
  selectedDatabase: string | null = null;
  loadingDatabases: boolean = false;

  // Tree navigation state
  databaseTree: NavTreeNode[] = [];
  selectedNodeId: string | null = null;
  selectedTable: string | null = null;
  treePanelWidth = 280;
  private readonly defaultCatalogKey = '__default__';
  private treeMinWidth = 220;
  private treeMaxWidth = 480;
  private isTreeResizing = false;
  private resizeStartX = 0;
  private resizeStartWidth = 280;
  private databaseCache: Record<string, string[]> = {};
  private tableCache: Record<string, string[]> = {};
  treePanelHeight: number = 420;
  private readonly treeExtraHeight: number = 140;
  treeCollapsed: boolean = false;
  private previousTreeWidth: number = this.treePanelWidth;
  readonly collapsedTreeWidth: number = 28;

  private buildNodeId(...parts: (string | undefined)[]): string {
    return parts
      .filter((part) => part !== undefined && part !== null)
      .map((part) => encodeURIComponent(part as string))
      .join('::');
  }

  private getCatalogKey(catalog?: string): string {
    return catalog && catalog.trim().length > 0 ? catalog : this.defaultCatalogKey;
  }

  private getDatabaseCacheKey(catalog: string, database: string): string {
    return `${this.getCatalogKey(catalog)}|${database}`;
  }

  private createCatalogNode(catalog: string): NavTreeNode {
    return {
      id: this.buildNodeId('catalog', catalog),
      name: catalog,
      type: 'catalog',
      icon: 'folder-outline',
      expanded: false,
      loading: false,
      children: [],
      data: {
        catalog,
      },
    };
  }

  private createDatabaseNode(catalog: string, database: string): NavTreeNode {
    return {
      id: this.buildNodeId('database', catalog, database),
      name: database,
      type: 'database',
      icon: 'cube-outline',
      expanded: false,
      loading: false,
      children: [],
      data: {
        catalog,
        database,
        originalName: database,
        tablesLoaded: false,
        tableCount: 0,
      },
    };
  }

  private createTableNode(catalog: string, database: string, table: string): NavTreeNode {
    return {
      id: this.buildNodeId('table', catalog, database, table),
      name: table,
      type: 'table',
      icon: 'grid-outline',
      expanded: false,
      loading: false,
      children: [],
      data: {
        catalog,
        database,
        table,
      },
    };
  }

  // Real-time query state
  sqlInput: string = '';
  queryResult: QueryExecuteResult | null = null;
  resultSettings: any[] = []; // Array of settings for multiple results
  executing: boolean = false;
  executionTime: number = 0;
  rowCount: number = 0;
  queryLimit: number = 1000; // Default limit for query results
  
  // Multiple query results
  queryResults: SingleQueryResult[] = [];
  resultSources: LocalDataSource[] = []; // Array of data sources for multiple results
  currentResultIndex: number = 0; // Track current selected tab index
  limitOptions = [
    { value: 100, label: '100 行' },
    { value: 500, label: '500 行' },
    { value: 1000, label: '1000 行' },
    { value: 5000, label: '5000 行' },
    { value: 10000, label: '10000 行' },
  ];
  
  // SQL Editor collapse state (default to expanded)
  sqlEditorCollapsed: boolean = false; // Default: expanded
  editorHeight: number = 400; // Default height
  
  // Running queries settings
  runningSettings = {
    mode: 'external',
    hideSubHeader: false, // Enable search
    noDataMessage: '当前没有运行中的查询',
    actions: false,
    pager: {
      display: true,
      perPage: 20,
    },
    columns: {
      QueryId: { title: 'Query ID', type: 'string' },
      User: { title: '用户', type: 'string', width: '10%' },
      Database: { title: '数据库', type: 'string', width: '10%' },
      ExecTime: { title: '执行时间', type: 'string', width: '10%' },
      Sql: { title: 'SQL', type: 'string' },
    },
  };

  constructor(
    private nodeService: NodeService,
    private route: ActivatedRoute,
    private toastrService: NbToastrService,
    private clusterContext: ClusterContextService,
    private themeService: NbThemeService,
  ) {
    // Try to get clusterId from route first (for direct navigation)
    const routeClusterId = parseInt(this.route.snapshot.paramMap.get('clusterId') || '0', 10);
    this.clusterId = routeClusterId;
  }

  ngAfterViewInit(): void {
    // Initialize CodeMirror editor after view is ready
    setTimeout(() => {
      this.initEditor();
      this.calculateEditorHeight();
      if (this.clusterId && this.clusterId > 0) {
        this.loadCatalogs();
      }
    }, 100);

    // Subscribe to theme changes
    this.themeService.onThemeChange()
      .pipe(takeUntil(this.destroy$))
      .subscribe((theme: any) => {
        this.currentTheme = theme.name;
        this.updateEditorTheme();
      });

    // Get current theme
    this.themeService.getJsTheme()
      .pipe(takeUntil(this.destroy$))
      .subscribe((theme: any) => {
        this.currentTheme = theme?.name || 'default';
        this.updateEditorTheme();
      });
    
  }
  
  @HostListener('window:resize', ['$event'])
  onResize(event: any) {
    this.calculateEditorHeight();
  }

  @HostListener('document:mousemove', ['$event'])
  onDocumentMouseMove(event: MouseEvent): void {
    if (!this.isTreeResizing) {
      return;
    }

    const delta = event.clientX - this.resizeStartX;
    const newWidth = this.resizeStartWidth + delta;
    this.treePanelWidth = Math.min(this.treeMaxWidth, Math.max(this.treeMinWidth, newWidth));
    event.preventDefault();
  }

  @HostListener('document:mouseup')
  onDocumentMouseUp(): void {
    if (this.isTreeResizing) {
      this.isTreeResizing = false;
      document.body.classList.remove('resizing-tree');
    }
  }
  
  updateCollapseToggleVisibility(): void {
    // This function is no longer needed as the button is always visible
  }

  toggleTreeCollapsed(): void {
    this.treeCollapsed = !this.treeCollapsed;
    if (this.treeCollapsed) {
      this.previousTreeWidth = this.treePanelWidth;
      this.treePanelWidth = this.collapsedTreeWidth;
    } else {
      const restoredWidth = Math.max(this.treeMinWidth, Math.min(this.treeMaxWidth, this.previousTreeWidth));
      this.treePanelWidth = restoredWidth;
    }
    setTimeout(() => this.calculateEditorHeight(), 0);
  }
  
  // Calculate dynamic editor height based on viewport
  calculateEditorHeight(): void {
    const windowHeight = window.innerHeight;
    const navbarHeight = 64; // Approximate navbar height
    const tabBarHeight = 48; // Tab bar height
    const editorToolbarHeight = 80; // Selection breadcrumbs + buttons + toolbar
    const bottomMargin = 16; // Small margin at bottom
    
    // Calculate available height
    let availableHeight = windowHeight - navbarHeight - tabBarHeight - editorToolbarHeight - bottomMargin;
    
    // If there are results, reserve space for them
    if (this.queryResult && !this.sqlEditorCollapsed) {
      availableHeight = Math.max(300, availableHeight * 0.4); // Editor takes 40% when results shown
    } else if (this.queryResult && this.sqlEditorCollapsed) {
      availableHeight = 60; // Collapsed bar height
    }
    
    this.editorHeight = Math.max(200, availableHeight);
    const calculatedTreeHeight = this.editorHeight + this.treeExtraHeight;
    this.treePanelHeight = Math.max(420, calculatedTreeHeight);
  }

  startTreeResize(event: MouseEvent): void {
    if (this.treeCollapsed) {
      return;
    }
    this.isTreeResizing = true;
    this.resizeStartX = event.clientX;
    this.resizeStartWidth = this.treePanelWidth;
    event.preventDefault();
    event.stopPropagation();
    document.body.classList.add('resizing-tree');
  }

  toggleNode(node: NavTreeNode, event?: MouseEvent): void {
    if (event) {
      event.stopPropagation();
      event.preventDefault();
    }

    if (node.loading) {
      return;
    }

    node.expanded = !node.expanded;

    if (!node.expanded) {
      return;
    }

    switch (node.type) {
      case 'catalog':
        this.loadDatabasesForCatalog(node);
        break;
      case 'group':
        // No longer used
        break;
      default:
        if (node.type === 'database') {
          this.loadTablesForDatabase(node);
        }
        break;
    }
  }

  onNodeSelect(node: NavTreeNode, event?: MouseEvent): void {
    if (event) {
      event.stopPropagation();
      event.preventDefault();
    }

    this.selectedNodeId = node.id;

    switch (node.type) {
      case 'catalog': {
        const catalogName = node.data?.catalog || '';
        this.setSelectedContext(catalogName, null, null);
        if (!node.expanded) {
          this.toggleNode(node);
        }
        break;
      }
      case 'database': {
        const catalogName = node.data?.catalog || '';
        const databaseName = node.data?.database || '';
        this.setSelectedContext(catalogName, databaseName, null);
        if (!node.expanded) {
          this.toggleNode(node);
        }
        if (!node.data?.tablesLoaded) {
          this.loadTablesForDatabase(node);
        }
        break;
      }
      case 'group': {
        // No longer used
        break;
      }
      case 'table': {
        const catalogName = node.data?.catalog || '';
        const databaseName = node.data?.database || '';
        const tableName = node.data?.table || '';
        this.setSelectedContext(catalogName, databaseName, tableName);
        break;
      }
      default:
        break;
    }
  }

  private setSelectedContext(catalog: string, database: string | null, table: string | null): void {
    this.selectedCatalog = catalog;
    this.selectedDatabase = database;
    this.selectedTable = table;

    if (this.editorView) {
      setTimeout(() => this.calculateEditorHeight(), 0);
    }
  }

  private resetNavigationState(): void {
    this.databaseTree = [];
    this.databaseCache = {};
    this.tableCache = {};
    this.selectedNodeId = null;
    this.selectedCatalog = '';
    this.selectedDatabase = null;
    this.selectedTable = null;
  }

  private loadDatabasesForCatalog(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const cacheKey = this.getCatalogKey(catalogName);

    if (this.databaseCache[cacheKey]) {
      node.children = this.databaseCache[cacheKey].map((db) => this.createDatabaseNode(catalogName, db));
      return;
    }

    node.loading = true;
    this.loadingDatabases = true;

    this.nodeService.getDatabases(catalogName || undefined).subscribe({
      next: (databases) => {
        const dbList = databases || [];
        this.databaseCache[cacheKey] = dbList;
        node.children = dbList.map((db) => this.createDatabaseNode(catalogName, db));
        node.loading = false;
        this.loadingDatabases = false;

        if (node.expanded && this.selectedNodeId === node.id && node.children.length > 0) {
          this.onNodeSelect(node.children[0]);
        }
      },
      error: (error) => {
        node.loading = false;
        this.loadingDatabases = false;
        console.error('Failed to load databases:', error);
        node.children = [];
        this.toastrService.danger('加载数据库列表失败', '错误');
      },
    });
  }

  private loadTablesForDatabase(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || '';

    if (!databaseName) {
      node.children = [];
      return;
    }

    const cacheKey = this.getDatabaseCacheKey(catalogName, databaseName);

    const applyTables = (tables: string[]) => {
      const tableList = tables || [];
      this.tableCache[cacheKey] = tableList;
      node.children = tableList.map((table) => this.createTableNode(catalogName, databaseName, table));
      const baseName = node.data?.originalName || databaseName;
      node.name = `${baseName} (${tableList.length})`;
      if (node.data) {
        node.data.tablesLoaded = true;
        node.data.tableCount = tableList.length;
      }
      if (node.expanded && this.selectedNodeId === node.id && node.children.length > 0) {
        this.onNodeSelect(node.children[0]);
      }
    };

    if (this.tableCache[cacheKey]) {
      applyTables(this.tableCache[cacheKey]);
      return;
    }

    node.loading = true;

    this.nodeService.getTables(catalogName || undefined, databaseName).subscribe({
      next: (tables) => {
        applyTables(tables || []);
        node.loading = false;
      },
      error: (error) => {
        node.loading = false;
        console.error('Failed to load tables:', error);
        node.children = [];
        const baseName = node.data?.originalName || databaseName || node.name;
        node.name = `${baseName}`;
        if (node.data) {
          node.data.tablesLoaded = false;
          node.data.tableCount = 0;
        }
        this.toastrService.danger(`加载表列表失败: ${error.message || error.statusText || '未知错误'}`, '错误');
      },
    });
  }

  getNodeIndent(node: NavTreeNode): number {
    switch (node.type) {
      case 'catalog':
        return 12;
      case 'database':
        return 32;
      case 'table':
        return 52;
      default:
        return 12;
    }
  }

  isNodeExpandable(node: NavTreeNode): boolean {
    return node.type !== 'table';
  }

  trackNodeById(index: number, node: NavTreeNode): string {
    return node.id;
  }

  private initEditor(): void {
    if (!this.editorContainer?.nativeElement) {
      return;
    }

    const isDark = this.currentTheme === 'dark' || this.currentTheme === 'cosmic';

    const extensions = [
      basicSetup,
      sql(),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          this.sqlInput = update.state.doc.toString();
        }
      }),
      EditorView.theme({
        '&': {
          height: `${this.editorHeight}px`,
          width: '100%', // Ensure full width
          fontSize: '20px',
          maxWidth: '100%',
          boxSizing: 'border-box',
        },
        '.cm-content': {
          fontSize: '20px',
          maxWidth: '100%',
          boxSizing: 'border-box',
        },
        '.cm-line': {
          fontSize: '20px',
          wordWrap: 'break-word',
          whiteSpace: 'pre-wrap',
        },
        '.cm-scroller': {
          overflowX: 'hidden', // Hide horizontal scrollbar
          overflowY: 'auto',
          width: '100%', // Ensure full width
          maxWidth: '100%',
          boxSizing: 'border-box',
        },
      }),
    ];

    if (isDark) {
      extensions.push(oneDark);
    }

    if (this.editorView) {
      this.editorView.destroy();
    }

    this.editorView = new EditorView({
      doc: this.sqlInput || '',
      extensions,
      parent: this.editorContainer.nativeElement,
    });
  }

  private updateEditorTheme(): void {
    const isDark = this.currentTheme === 'dark' || this.currentTheme === 'cosmic';
    
    setTimeout(() => this.calculateEditorHeight(), 0);
  }

  private destroyEditor(): void {
    if (this.editorView) {
      this.editorView.destroy();
      this.editorView = null;
    }
  }

  private loadCatalogs(autoSelectFirst = true): void {
    if (!this.clusterId) {
      return;
    }

    this.loadingCatalogs = true;
    this.nodeService.getCatalogs().subscribe({
      next: (catalogs) => {
        const catalogList = (catalogs || []).filter((name) => !!name && name.trim().length > 0);
        catalogList.sort((a, b) => a.localeCompare(b));
        this.catalogs = catalogList;
        this.loadingCatalogs = false;
        this.databaseTree = this.catalogs.map((catalog) => this.createCatalogNode(catalog));

        if (autoSelectFirst && this.databaseTree.length > 0) {
          const firstCatalogNode = this.databaseTree[0];
          this.onNodeSelect(firstCatalogNode);
          this.toggleNode(firstCatalogNode);
        }
      },
      error: (error) => {
        this.loadingCatalogs = false;
        console.error('Failed to load catalogs:', error);
      },
    });
  }

  ngOnInit(): void {
    // Subscribe to active cluster changes
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        this.activeCluster = cluster;
        if (cluster) {
          // Always use the active cluster (override route parameter)
          const newClusterId = cluster.id;
          if (this.clusterId !== newClusterId) {
            this.clusterId = newClusterId;
            // Load catalogs when cluster changes (this will auto-select and load databases)
            this.resetNavigationState();
            this.loadCatalogs();
            // Only load if not on realtime tab
            if (this.selectedTab !== 'realtime') {
              this.loadCurrentTab();
            } else {
              this.loading = false;
            }
          }
        }
      });

    // Load queries if clusterId is already set from route
    if (this.clusterId && this.clusterId > 0) {
      // Only load if not on realtime tab
      if (this.selectedTab !== 'realtime') {
        this.loadCurrentTab();
      } else {
        this.loading = false;
      }
    }
  }

  ngOnDestroy(): void {
    this.stopAutoRefresh();
    this.destroyEditor();
    this.destroy$.next();
    this.destroy$.complete();
    document.body.classList.remove('resizing-tree');
  }

  // Tab switching
  selectTab(tab: string): void {
    this.selectedTab = tab;
    this.loadCurrentTab();
  }

  // Auto refresh methods
  toggleAutoRefresh(): void {
    this.autoRefresh = !this.autoRefresh;
    if (this.autoRefresh) {
      this.startAutoRefresh();
    } else {
      this.stopAutoRefresh();
    }
  }

  onRefreshIntervalChange(interval: number): void {
    this.selectedRefreshInterval = interval;
    if (this.autoRefresh) {
      // Restart with new interval
      this.stopAutoRefresh();
      this.startAutoRefresh();
    }
  }

  startAutoRefresh(): void {
    this.stopAutoRefresh(); // Clear any existing interval
    this.refreshInterval = setInterval(() => {
      this.loadCurrentTab();
    }, this.selectedRefreshInterval * 1000);
  }

  stopAutoRefresh(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  // Load data based on current tab
  loadCurrentTab(): void {
    if (this.selectedTab === 'running') {
      this.loadRunningQueries();
    } else {
      // realtime tab doesn't need auto-loading
      this.loading = false;
    }
  }

  // Load running queries
  loadRunningQueries(): void {
    this.loading = true;
    this.nodeService.listQueries().subscribe({
      next: (queries) => {
        this.runningSource.load(queries);
        this.loading = false;
      },
      error: (error) => {
        this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '加载失败');
        this.loading = false;
      },
    });
  }

  // Toggle SQL editor collapse state
  toggleSqlEditor(collapsed?: boolean): void {
    if (collapsed !== undefined) {
      this.sqlEditorCollapsed = collapsed;
    } else {
      this.sqlEditorCollapsed = !this.sqlEditorCollapsed;
    }

    if (!this.sqlEditorCollapsed) {
      // Simply recalculate height to make sure layout is correct after expansion
      setTimeout(() => this.calculateEditorHeight(), 300);
    }
  }

  // Real-time query methods
  executeSQL(): void {
    if (!this.sqlInput || this.sqlInput.trim() === '') {
      this.toastrService.warning('请输入SQL语句', '提示');
      return;
    }

    // Check if catalog is selected
    if (!this.selectedCatalog) {
      this.toastrService.warning('请先选择 Catalog', '提示');
      return;
    }

    // Check if databases are still loading
    if (this.loadingDatabases) {
      this.toastrService.warning('数据库列表加载中，请稍候...', '提示');
      return;
    }

    if (!this.selectedDatabase) {
        this.toastrService.warning('请选择数据库', '提示');
        return;
    }

    this.executing = true;
    this.queryResult = null;
    this.resultSettings = [];
    this.queryResults = [];
    this.resultSources = [];
    this.currentResultIndex = 0;

    this.nodeService.executeSQL(
      this.sqlInput.trim(), 
      this.queryLimit, 
      this.selectedCatalog || undefined,
      this.selectedDatabase || undefined
    ).subscribe({
      next: (result) => {
        this.queryResult = result;
        this.queryResults = result.results;
        this.executionTime = result.total_execution_time_ms;
        
        // Build settings and data sources for each result
        this.resultSettings = [];
        this.resultSources = [];
        
        let totalRowCount = 0;
        let successCount = 0;
        
        result.results.forEach((singleResult, index) => {
          if (singleResult.success) {
            successCount++;
            totalRowCount += singleResult.row_count;
            
            // Build dynamic table settings for this result
            const settings = this.buildResultSettings(singleResult);
            this.resultSettings.push(settings);
            
            // Convert rows to objects for ng2-smart-table
            const dataRows = singleResult.rows.map(row => {
              const obj: any = {};
              singleResult.columns.forEach((col, idx) => {
                obj[col] = row[idx];
              });
              return obj;
            });
            
            const source = new LocalDataSource();
            source.load(dataRows);
            this.resultSources.push(source);
          } else {
            // For failed queries, still add placeholder settings and empty source
            this.resultSettings.push(null);
            const source = new LocalDataSource();
            source.load([]);
            this.resultSources.push(source);
          }
        });
        
        this.rowCount = totalRowCount;
        this.executing = false;
        
        if (result.results.length > 1) {
          this.toastrService.success(
            `执行 ${result.results.length} 个SQL，成功 ${successCount} 个，共返回 ${totalRowCount} 行`,
            '成功'
          );
        } else {
          const singleResult = result.results[0];
          if (singleResult.success) {
            this.toastrService.success(`查询成功，返回 ${singleResult.row_count} 行`, '成功');
          } else {
            this.toastrService.danger(singleResult.error || '执行失败', '执行失败');
          }
        }
        
        // Auto-collapse SQL editor after successful query
        if (result.results.length > 0 && result.results[0].success) {
          setTimeout(() => {
            this.toggleSqlEditor(true);
          }, 300); // Delay for smooth UX
        }
      },
      error: (error) => {
        this.executing = false;
        this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '执行失败');
      },
    });
  }

  buildResultSettings(result: SingleQueryResult): any {
    const columns: any = {};
    result.columns.forEach(col => {
      columns[col] = { title: col, type: 'string' };
    });

    return {
      mode: 'external',
      hideSubHeader: false, // Enable search
      noDataMessage: '无数据',
      actions: false,
      pager: {
        display: true,
        perPage: 50,
      },
      columns: columns,
    };
  }

  // Generate tab title
  getResultTabTitle(result: SingleQueryResult, index: number): string {
    return `结果${index + 1}`;
  }

  clearSQL(): void {
    this.sqlInput = '';
    if (this.editorView) {
      const transaction = this.editorView.state.update({
        changes: {
          from: 0,
          to: this.editorView.state.doc.length,
          insert: '',
        },
      });
      this.editorView.dispatch(transaction);
    }
    this.queryResult = null;
    this.resultSettings = [];
    this.queryResults = [];
    this.resultSources = [];
    this.executionTime = 0;
    this.rowCount = 0;
  }

  formatSQL(): void {
    if (!this.sqlInput) {
      return;
    }
    try {
      // Use sql-formatter for proper SQL formatting
      const formatted = format(this.sqlInput.trim(), {
        language: 'sql',
        tabWidth: 2,
        keywordCase: 'upper',
        identifierCase: 'lower',
      });
      
      this.sqlInput = formatted;
      
      // Update editor content
      if (this.editorView) {
        const transaction = this.editorView.state.update({
          changes: {
            from: 0,
            to: this.editorView.state.doc.length,
            insert: formatted,
          },
        });
        this.editorView.dispatch(transaction);
      }
    } catch (error) {
      this.toastrService.warning('格式化失败，使用原始SQL', '提示');
    }
  }

  // Export results to CSV
  exportResults(resultIndex?: number): void {
    let resultToExport: SingleQueryResult | null = null;
    
    if (resultIndex !== undefined && this.queryResults[resultIndex]) {
      // Export specific result from multiple results
      resultToExport = this.queryResults[resultIndex];
    } else if (this.queryResults.length === 1) {
      // Export single result
      resultToExport = this.queryResults[0];
    } else {
      this.toastrService.warning('请选择要导出的结果', '提示');
      return;
    }
    
    if (!resultToExport || !resultToExport.success || !resultToExport.rows || resultToExport.rows.length === 0) {
      this.toastrService.warning('没有数据可导出', '提示');
      return;
    }

    try {
      // Build CSV content
      const columns = resultToExport.columns;
      const rows = resultToExport.rows;

      // CSV header
      let csvContent = columns.map(col => this.escapeCSV(col)).join(',') + '\n';

      // CSV rows
      rows.forEach(row => {
        csvContent += row.map(cell => this.escapeCSV(cell)).join(',') + '\n';
      });

      // Create blob and download
      const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
      const link = document.createElement('a');
      const url = URL.createObjectURL(blob);
      
      const filename = resultIndex !== undefined 
        ? `query_result_${resultIndex + 1}_${new Date().getTime()}.csv`
        : `query_result_${new Date().getTime()}.csv`;
      
      link.setAttribute('href', url);
      link.setAttribute('download', filename);
      link.style.visibility = 'hidden';
      
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);

      this.toastrService.success('导出成功', '成功');
    } catch (error) {
      console.error('Export error:', error);
      this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '导出失败');
    }
  }

  // Escape CSV special characters
  private escapeCSV(value: string): string {
    if (value === null || value === undefined) {
      return '';
    }
    const stringValue = String(value);
    if (stringValue.includes(',') || stringValue.includes('"') || stringValue.includes('\n')) {
      return '"' + stringValue.replace(/"/g, '""') + '"';
    }
    return stringValue;
  }
}

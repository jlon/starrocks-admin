import { Component, OnInit, OnDestroy, ViewChild, AfterViewInit, ElementRef, HostListener, TemplateRef } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { NbDialogRef, NbDialogService, NbMenuItem, NbMenuService, NbToastrService, NbThemeService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject, Observable, forkJoin, of } from 'rxjs';
import { map, catchError, takeUntil } from 'rxjs/operators';
import { NodeService, QueryExecuteResult, SingleQueryResult, TableInfo, TableObjectType } from '../../../../@core/data/node.service';
import { ClusterContextService } from '../../../../@core/data/cluster-context.service';
import { Cluster } from '../../../../@core/data/cluster.service';
import { ErrorHandler } from '../../../../@core/utils/error-handler';
import { EditorView } from '@codemirror/view';
import { autocompletion, completionKeymap, Completion, CompletionSource } from '@codemirror/autocomplete';
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search';
import { highlightActiveLine, highlightActiveLineGutter, drawSelection, keymap } from '@codemirror/view';
import { syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language';
import { EditorState, Compartment, type Extension } from '@codemirror/state';
import { history, historyKeymap } from '@codemirror/commands';
import { closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete';
import { sql, MySQL, type SQLNamespace } from '@codemirror/lang-sql';
import { format } from 'sql-formatter';
import { trigger, transition, style, animate, state } from '@angular/animations';
import { renderMetricBadge, MetricThresholds } from '../../../../@core/utils/metric-badge';
import { renderLongText } from '../../../../@core/utils/text-truncate';
import { ConfirmDialogService } from '../../../../@core/services/confirm-dialog.service';
import { AuthService } from '../../../../@core/data/auth.service';

type NavNodeType = 'catalog' | 'database' | 'group' | 'table';

type ContextMenuAction = 
  | 'viewSchema' 
  | 'viewPartitions'
  | 'viewTransactions'      // 数据库/表级别
  | 'viewCompactions'       // 数据库/表级别
  | 'viewLoads'            // 数据库级别
  | 'viewDatabaseStats'    // 数据库级别
  | 'viewTableStats'       // 表级别
  | 'viewCompactionScore'  // 表级别
  | 'triggerCompaction'    // 表级别 - 手动触发Compaction
  | 'cancelCompaction';    // Compaction任务 - 取消任务

interface TreeContextMenuItem {
  label: string;
  icon: string;
  action: ContextMenuAction;
}

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
    tableType?: TableObjectType;
    originalName?: string;
    tablesLoaded?: boolean;
    tableCount?: number;
    dbId?: string; // Cached database ID
  };
}

@Component({
  selector: 'ngx-query-execution',
  templateUrl: './query-execution.component.html',
  styleUrls: ['./query-execution.component.scss'],
  animations: [
    trigger('editorCollapse', [
      state('expanded', style({ height: '*', opacity: 1, overflow: 'visible' })),
      state('collapsed', style({ 
        height: '0px', 
        opacity: 0, 
        paddingTop: 0, 
        paddingBottom: 0, 
        marginTop: 0,
        marginBottom: 0, 
        overflow: 'hidden' 
      })),
      transition('expanded <=> collapsed', animate('200ms ease')),
    ]),
  ],
})
export class QueryExecutionComponent implements OnInit, OnDestroy, AfterViewInit {
  @ViewChild('editorContainer', { static: false }) editorContainer!: ElementRef;
  @ViewChild('tableSchemaDialog', { static: false }) tableSchemaDialogTemplate!: TemplateRef<any>;
  @ViewChild('infoDialog', { static: false }) infoDialogTemplate!: TemplateRef<any>;
  @ViewChild('compactionTriggerDialog', { static: false }) compactionTriggerDialogTemplate!: TemplateRef<any>;

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
  private tableCache: Record<string, TableInfo[]> = {};
  // Cache database ID mapping: catalog|database -> dbId
  private databaseIdCache: Record<string, string> = {};
  private currentSqlSchema: SQLNamespace = {};
  treePanelHeight: number = 420;
  private readonly treeExtraHeight: number = 140;
  treeCollapsed: boolean = false;
  private previousTreeWidth: number = this.treePanelWidth;
  readonly collapsedTreeWidth: number = 28;
  private readonly sqlDialect = MySQL;
  private readonly themeCompartment = new Compartment();
  private readonly sqlConfigCompartment = new Compartment();
  private readonly runningDurationThresholds: MetricThresholds = { warn: 3000, danger: 10000 };

  // Table schema dialog state
  schemaDialogTitle: string = '';
  schemaDialogSubtitle: string = '';
  currentSchemaCatalog: string | null = null;
  currentSchemaDatabase: string | null = null;
  currentSchemaTable: string | null = null;
  currentTableSchema: string = '';
  tableSchemaLoading: boolean = false;
  private schemaDialogRef: NbDialogRef<any> | null = null;

  // Info dialog state (for transactions, compactions, loads, stats, etc.)
  private infoDialogRef: NbDialogRef<any> | null = null;
  infoDialogTitle: string = '';
  infoDialogData: any[] = [];
  infoDialogLoading: boolean = false;
  infoDialogError: string | null = null;
  infoDialogType: 'transactions' | 'compactions' | 'compactionDetails' | 'loads' | 'databaseStats' | 'tableStats' | 'partitions' | 'compactionScore' | null = null;
  infoDialogSettings: any = {};
  infoDialogSource: LocalDataSource = new LocalDataSource();
  
  // Page-level loading state for info dialogs
  infoDialogPageLoading: boolean = false;
  
  // Pagination settings for info dialogs
  infoDialogPerPage: number = 15;
  perPageOptions = [10, 15, 20, 30, 50, 100];
  
  // Transaction dialog state (for tab switching)
  transactionRunningData: any[] = [];
  transactionFinishedData: any[] = [];
  transactionCurrentTab: 'running' | 'finished' = 'running';
  transactionColumns: any = {};
  
  // Compaction trigger dialog state
  compactionTriggerDialogRef: NbDialogRef<any> | null = null;
  compactionTriggerTable: string | null = null;
  compactionTriggerDatabase: string | null = null;
  compactionTriggerCatalog: string | null = null;
  compactionSelectedPartitions: string[] = [];
  compactionTriggerMode: 'table' | 'partition' = 'table';
  compactionTriggering: boolean = false;
  availablePartitions: string[] = [];
  contextMenuVisible: boolean = false;
  contextMenuItems: TreeContextMenuItem[] = [];
  contextMenuX = 0;
  contextMenuY = 0;
  private contextMenuTargetNode: NavTreeNode | null = null;

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
        dbId: undefined, // Will be populated when loading databases
      },
    };
  }

  private createTableNode(catalog: string, database: string, table: TableInfo): NavTreeNode {
    const icon = this.getTableIcon(table.object_type);
    return {
      id: this.buildNodeId('table', catalog, database, table.name),
      name: table.name,
      type: 'table',
      icon,
      expanded: false,
      loading: false,
      children: [],
      data: {
        catalog,
        database,
        table: table.name,
        tableType: table.object_type,
      },
    };
  }

  private getTableIcon(tableType: TableObjectType): string {
    switch (tableType) {
      case 'VIEW':
        return 'eye-outline';
      case 'MATERIALIZED_VIEW':
        return 'layers-outline';
      default:
        return 'grid-outline';
    }
  }

  private mapTableNames(tables: TableInfo[]): string[] {
    return tables.map((table) => table.name).filter((name) => !!name);
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
      ExecTime: {
        title: '执行时间(ms)',
        type: 'html',
        width: '12%',
        valuePrepareFunction: (value: string | number) => renderMetricBadge(value, this.runningDurationThresholds),
      },
      Sql: { title: 'SQL', type: 'string' },
    },
  };

  constructor(
    private nodeService: NodeService,
    private route: ActivatedRoute,
    private toastrService: NbToastrService,
    private clusterContext: ClusterContextService,
    private themeService: NbThemeService,
    private dialogService: NbDialogService,
    private confirmDialogService: ConfirmDialogService,
    private authService: AuthService,
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

  @HostListener('document:click', ['$event'])
  onDocumentClick(event: MouseEvent): void {
    if (!this.contextMenuVisible) {
      return;
    }

    const target = event.target as HTMLElement | null;
    if (target && target.closest('.tree-context-menu')) {
      return;
    }

    this.closeContextMenu();
  }

  @HostListener('window:scroll')
  onWindowScroll(): void {
    if (this.contextMenuVisible) {
      this.closeContextMenu();
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
    const cardHeaderHeight = 56; // nb-card header
    const cardPadding = 32; // nb-card-body padding
    const treeHeaderHeight = 48; // Tree panel header height
    const bottomMargin = 16; // Small margin at bottom
    
    // Calculate tree panel height to stretch to bottom
    const availableTreeHeight = windowHeight - navbarHeight - tabBarHeight - cardHeaderHeight - cardPadding - bottomMargin;
    this.treePanelHeight = Math.max(300, availableTreeHeight);
    
    // Calculate editor height based on tree height
    const editorToolbarHeight = 80; // Selection breadcrumbs + buttons
    const editorFooterHeight = 28; // Footer with limit selector
    
    if (this.sqlEditorCollapsed) {
      this.editorHeight = 0;
      if (this.editorView) {
        this.applyEditorTheme();
      }
      return;
    }

    // If there are results, reserve space for them
    if (this.queryResult) {
      const editorAvailableHeight = this.treePanelHeight - treeHeaderHeight - editorToolbarHeight - editorFooterHeight;
      this.editorHeight = Math.max(200, editorAvailableHeight * 0.4); // Editor takes 40% when results shown
    } else {
      // No results, editor takes more space
      const editorAvailableHeight = this.treePanelHeight - treeHeaderHeight - editorToolbarHeight - editorFooterHeight;
      this.editorHeight = Math.max(200, editorAvailableHeight);
    }
    
    if (this.editorView) {
      this.applyEditorTheme();
    }
  }


  private applyEditorTheme(): void {
    if (!this.editorView) {
      return;
    }
    this.editorView.dispatch({
      effects: this.themeCompartment.reconfigure(this.buildEditorTheme()),
    });
    }

  private buildEditorTheme(): Extension {
    const isDark = this.currentTheme === 'dark' || this.currentTheme === 'cosmic';
    const palette = isDark
      ? {
          background: '#202632',
          gutter: '#1a2130',
          gutterBorder: 'transparent',
          lineNumber: '#8392c1',
          keyword: '#8bb2ff',
          string: '#4fd19d',
        }
      : {
          background: '#FCFEFF',
          gutter: '#EFF2FB',
          gutterBorder: '#E6EAF5',
          lineNumber: '#8C9BC5',
          keyword: '#3366FF',
          string: '#2BAE66',
        };

    return EditorView.theme(
      {
        '&': {
          height: `${this.editorHeight}px`,
          backgroundColor: palette.background,
        },
        '.cm-content': {
          padding: '8px 12px',
          fontSize: '14px',
        },
        '.cm-line': {
          fontFamily: `'JetBrains Mono', Menlo, Consolas, monospace`,
          fontSize: '14px',
        },
        '.cm-gutters': {
          backgroundColor: palette.gutter,
          borderRight: palette.gutterBorder,
          color: palette.lineNumber,
        },
        '.cm-selectionBackground, .cm-selectionLayer .cm-selectionBackground': {
          backgroundColor: isDark ? 'rgba(104, 125, 191, 0.35)' : 'rgba(51, 102, 255, 0.16)',
        },
        '.cm-matchingBracket': {
          outline: `1px solid ${palette.keyword}`,
        },
      },
      { dark: isDark },
    );
  }

  private applySqlSchema(): void {
    if (!this.editorView) {
      return;
    }
    this.editorView.dispatch({
      effects: this.sqlConfigCompartment.reconfigure(
        sql({
          dialect: this.sqlDialect,
          upperCaseKeywords: true,
          schema: this.currentSqlSchema,
        }),
      ),
    });
  }

  private buildSqlSchema(): SQLNamespace {
    const namespace: Record<string, SQLNamespace> = {};

    this.databaseTree.forEach((catalogNode) => {
      const catalogName = catalogNode.data?.catalog || catalogNode.name || '';
      const catalogKey = this.getCatalogKey(catalogName);
      const databases = this.databaseCache[catalogKey] || [];

      if (databases.length === 0) {
        if (catalogName) {
          namespace[catalogName] = [];
        }
        return;
      }

      const dbNamespace: Record<string, SQLNamespace> = {};
      databases.forEach((databaseName) => {
        const tableKey = this.getDatabaseCacheKey(catalogName, databaseName);
        const tables = this.tableCache[tableKey] || [];
        const tableNames = this.mapTableNames(tables);
        dbNamespace[databaseName] = tableNames.length > 0 ? tableNames : [];
      });

      namespace[catalogName || 'default'] = dbNamespace;
    });

    if (Object.keys(namespace).length === 0 && Object.keys(this.tableCache).length > 0) {
      const fallback: Record<string, SQLNamespace> = {};
      Object.entries(this.tableCache).forEach(([cacheKey, tables]) => {
        const [, databaseName] = cacheKey.split('|');
        if (databaseName) {
          const tableNames = this.mapTableNames(tables);
          fallback[databaseName] = tableNames.length > 0 ? tableNames : [];
        }
      });
      if (Object.keys(fallback).length > 0) {
        namespace['default'] = fallback;
      }
    }

    return namespace;
  }

  private refreshSqlSchema(): void {
    this.currentSqlSchema = this.buildSqlSchema();
    this.applySqlSchema();
  }

  private buildSchemaCompletions(context: any): { completions: Completion[], from: number } | null {
    if (Object.keys(this.currentSqlSchema).length === 0) {
      return null;
    }

    const completions: Completion[] = [];
    const { state, pos } = context;
    
    // Get text before cursor
    const textBefore = state.doc.sliceString(Math.max(0, pos - 200), pos);
    
    // Parse the context to find dot notation prefix (e.g., "catalog.database." or "database.")
    // Match patterns like "identifier." or "catalog.database." at the end of text before cursor
    // Also capture any partial identifier being typed after the dot
    const dotMatch = textBefore.match(/([\w\u4e00-\u9fa5.]+)\.(\w*)$/);
    
    // ONLY provide schema completions when after a dot
    // This prevents schema items from interfering with keyword completions
    if (!dotMatch || !dotMatch[1]) {
      return null;
    }
    
    // Found dot notation, parse the path
    const pathParts = dotMatch[1].split('.').filter(p => p.trim());
    const prefixPath = pathParts;
    const partialWord = dotMatch[2] || ''; // The partial word being typed after the dot
    const wordStartPos = partialWord ? pos - partialWord.length : pos; // Start position for replacement
    
    // Navigate to the target namespace
    let targetNamespace: SQLNamespace | null = this.currentSqlSchema;
    
    // If only one path part (e.g., "sys."), it could be a database name
    // Try to find it in all catalogs, prioritizing the selected catalog
    if (pathParts.length === 1) {
      const dbName = pathParts[0];
      let foundNamespace: SQLNamespace | null = null;
      
      // First, try to find in the selected catalog if available
      if (this.selectedCatalog && this.currentSqlSchema[this.selectedCatalog]) {
        const catalogNs = this.currentSqlSchema[this.selectedCatalog] as SQLNamespace;
        if (catalogNs && typeof catalogNs === 'object' && !Array.isArray(catalogNs) && catalogNs[dbName]) {
          foundNamespace = catalogNs[dbName] as SQLNamespace;
        }
      }
      
      // If not found in selected catalog, search all catalogs
      if (!foundNamespace) {
        for (const catalogKey in this.currentSqlSchema) {
          if (Object.prototype.hasOwnProperty.call(this.currentSqlSchema, catalogKey)) {
            const catalogNs = this.currentSqlSchema[catalogKey] as SQLNamespace;
            if (catalogNs && typeof catalogNs === 'object' && !Array.isArray(catalogNs) && catalogNs[dbName]) {
              foundNamespace = catalogNs[dbName] as SQLNamespace;
              break;
            }
          }
        }
      }
      
      if (!foundNamespace) {
      } else {
        if (Array.isArray(foundNamespace)) {
        }
      }
      
      targetNamespace = foundNamespace;
    } else {
      // Multiple path parts (e.g., "catalog.database."), navigate normally
      for (const part of pathParts) {
        if (targetNamespace && typeof targetNamespace === 'object' && !Array.isArray(targetNamespace)) {
          targetNamespace = (targetNamespace as SQLNamespace)[part] as SQLNamespace;
          if (!targetNamespace) {
            // Path not found in schema, return null
            return null;
          }
        } else {
          targetNamespace = null;
          break;
        }
      }
    }

    if (!targetNamespace) {
      return null;
    }

    // Handle when targetNamespace is a table array (e.g., after "database.")
    if (Array.isArray(targetNamespace)) {
      targetNamespace.forEach((tableName) => {
        if (!partialWord || tableName.toLowerCase().startsWith(partialWord.toLowerCase())) {
          completions.push({
            label: tableName,
            detail: prefixPath.join('.') || undefined,
            type: 'variable',
          });
        }
      });
      return completions.length > 0 ? { completions, from: wordStartPos } : null;
    }

    // targetNamespace is an object (catalog or contains databases)
    if (typeof targetNamespace !== 'object') {
      return null;
    }

    // Build completions from target namespace
    const processNamespace = (ns: SQLNamespace, path: string[] = []): void => {
      for (const key in ns) {
        if (Object.prototype.hasOwnProperty.call(ns, key)) {
          const value = ns[key];
          const currentPath = [...path, key];
          
          if (Array.isArray(value)) {
            // Tables array
            value.forEach((item) => {
              // Filter by partial word if exists
              if (!partialWord || item.toLowerCase().startsWith(partialWord.toLowerCase())) {
                completions.push({
                  label: item,
                  detail: currentPath.slice(0, -1).join('.') || undefined,
                  type: 'variable',
                });
              }
            });
          } else if (typeof value === 'object' && value !== null) {
            // Nested namespace (catalog/database)
            // Filter by partial word if exists
            if (!partialWord || key.toLowerCase().startsWith(partialWord.toLowerCase())) {
              completions.push({
                label: key,
                detail: currentPath.slice(0, -1).join('.') || undefined,
                type: 'namespace',
              });
            }
          }
        }
      }
    };
    
    processNamespace(targetNamespace, prefixPath);
    return completions.length > 0 ? { completions, from: wordStartPos } : null;
  }

  private buildKeywordCompletions(context: any): { completions: Completion[], from: number } | null {
    // SQL keywords that should be suggested
    const sqlKeywords = [
      'SELECT', 'FROM', 'WHERE', 'JOIN', 'INNER', 'LEFT', 'RIGHT', 'FULL', 'OUTER',
      'ON', 'AS', 'AND', 'OR', 'NOT', 'IN', 'EXISTS', 'LIKE', 'BETWEEN', 'IS', 'NULL',
      'GROUP', 'BY', 'HAVING', 'ORDER', 'ASC', 'DESC', 'LIMIT', 'OFFSET',
      'INSERT', 'INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE', 'CREATE', 'DROP',
      'ALTER', 'TABLE', 'DATABASE', 'INDEX', 'VIEW', 'TRIGGER', 'PROCEDURE',
      'UNION', 'ALL', 'DISTINCT', 'COUNT', 'SUM', 'AVG', 'MAX', 'MIN',
      'CASE', 'WHEN', 'THEN', 'ELSE', 'END', 'IF', 'ELSEIF',
      'CAST', 'CONVERT', 'NULLIF', 'COALESCE',
      'WITH', 'RECURSIVE',
      'EXPLAIN', 'DESCRIBE', 'SHOW', 'USE',
    ];

    const { state, pos } = context;
    const textBefore = state.doc.sliceString(Math.max(0, pos - 100), pos);
    
    // Don't suggest keywords if we're after a dot or in a quoted string
    const isAfterDot = /[\w.]\.\s*$/.test(textBefore);
    const isInQuotedString = /(['"])(?:[^\\]|\\.)*$/.test(textBefore);
    
    if (isAfterDot || isInQuotedString) {
      return null;
    }

    // Extract the current word being typed (if any)
    // Match word characters including those that may be part of SQL identifiers
    const wordMatch = textBefore.match(/([a-zA-Z_][a-zA-Z0-9_]*)$/);
    const currentWord = wordMatch ? wordMatch[1].toUpperCase() : '';
    const wordStartPos = wordMatch ? pos - wordMatch[1].length : pos;
    
    // If user is typing a word, filter keywords that start with it
    if (currentWord) {
      const matchingKeywords = sqlKeywords.filter(keyword => 
        keyword.startsWith(currentWord)
      );
      
      if (matchingKeywords.length > 0) {
        return {
          completions: matchingKeywords.map(keyword => ({
            label: keyword,
            type: 'keyword',
          })),
          from: wordStartPos, // Start from the beginning of the current word
        };
      }
    }

    // If no word is being typed or no matching keywords, check if we should show all keywords
    // Show all keywords at statement start (for explicit requests or clear boundaries)
    const isStatementStart = /(^|\s|;|,|\(|\))\s*$/.test(textBefore);
    
    if (isStatementStart) {
      // For explicit completions (Ctrl+Space) always show keywords
      if (context.explicit) {
        return {
          completions: sqlKeywords.map(keyword => ({
            label: keyword,
            type: 'keyword',
          })),
          from: pos,
        };
      }
      
      // For auto-trigger, only show at very clear statement boundaries
      const clearBoundary = /(^|[\s;])\s*$/.test(textBefore);
      if (clearBoundary) {
        return {
          completions: sqlKeywords.map(keyword => ({
            label: keyword,
            type: 'keyword',
          })),
          from: pos,
        };
      }
    }

    return null;
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

  onNodeRightClick(node: NavTreeNode, event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();

    this.contextMenuTargetNode = node;
    this.selectedNodeId = node.id;

    const items = this.buildContextMenuItems(node);
    if (!items || items.length === 0) {
      this.closeContextMenu();
      return;
    }

    this.contextMenuItems = items;
    const { x, y } = this.calculateContextMenuPosition(event, items.length);
    this.contextMenuX = x;
    this.contextMenuY = y;
    this.contextMenuVisible = true;
  }

  private calculateContextMenuPosition(event: MouseEvent, itemCount: number): { x: number; y: number } {
    const menuWidth = 200;
    const menuHeight = itemCount * 40 + 16;
    let x = event.clientX;
    let y = event.clientY;

    if (x + menuWidth > window.innerWidth - 8) {
      x = Math.max(8, window.innerWidth - menuWidth - 8);
    }

    if (y + menuHeight > window.innerHeight - 8) {
      y = Math.max(8, window.innerHeight - menuHeight - 8);
    }

    return { x, y };
  }

  private buildContextMenuItems(node: NavTreeNode): TreeContextMenuItem[] {
    if (node.type === 'database') {
      return [
        {
          label: '查看事务信息',
          icon: 'activity-outline',
          action: 'viewTransactions',
        },
        {
          label: '查看Compaction信息',
          icon: 'layers-outline',
          action: 'viewCompactions',
        },
        {
          label: '查看导入作业',
          icon: 'upload-outline',
          action: 'viewLoads',
        },
        {
          label: '查看数据库统计',
          icon: 'bar-chart-outline',
          action: 'viewDatabaseStats',
        },
      ];
    }

    if (node.type === 'table') {
      const tableType = node.data?.tableType;
      
      // View (视图) - 只有逻辑结构，没有物理存储
      if (tableType === 'VIEW') {
        return [
          {
            label: '查看视图结构',
            icon: 'file-text-outline',
            action: 'viewSchema',
          },
        ];
      }
      
      // Materialized View (物化视图) - 有物理存储，但Compaction由系统管理
      if (tableType === 'MATERIALIZED_VIEW') {
        return [
          {
            label: '查看物化视图结构',
            icon: 'file-text-outline',
            action: 'viewSchema',
          },
          {
            label: '查看分区',
            icon: 'layers-outline',
            action: 'viewPartitions',
          },
          {
            label: '查看Compaction Score',
            icon: 'trending-up-outline',
            action: 'viewCompactionScore',
          },
          {
            label: '查看表统计',
            icon: 'bar-chart-outline',
            action: 'viewTableStats',
          },
          {
            label: '查看表事务',
            icon: 'activity-outline',
            action: 'viewTransactions',
          },
        ];
      }
      
      // Regular Table (普通表) - 所有功能
      return [
        {
          label: '查看表结构',
          icon: 'file-text-outline',
          action: 'viewSchema',
        },
        {
          label: '查看分区',
          icon: 'layers-outline',
          action: 'viewPartitions',
        },
        {
          label: '查看Compaction Score',
          icon: 'trending-up-outline',
          action: 'viewCompactionScore',
        },
        {
          label: '查看表统计',
          icon: 'bar-chart-outline',
          action: 'viewTableStats',
        },
        {
          label: '查看表事务',
          icon: 'activity-outline',
          action: 'viewTransactions',
        },
        {
          label: '手动触发Compaction',
          icon: 'flash-outline',
          action: 'triggerCompaction',
        },
      ];
    }

    return [];
  }

  private handleContextMenuAction(item: TreeContextMenuItem): void {
    const action = item?.action;
    const targetNode = this.contextMenuTargetNode;

    if (!action || !targetNode) {
      this.closeContextMenu();
      return;
    }

    switch (action) {
      case 'viewSchema':
        if (targetNode.type === 'table') {
          this.viewTableSchema(targetNode);
        }
        break;
      case 'viewPartitions':
        if (targetNode.type === 'table') {
          // Views don't have partitions
          if (targetNode.data?.tableType === 'VIEW') {
            this.toastrService.warning('视图没有分区信息', '提示');
            return;
          }
          this.viewTablePartitions(targetNode);
        }
        break;
      case 'viewTransactions':
        if (targetNode.type === 'database') {
          this.viewDatabaseTransactions(targetNode);
        } else if (targetNode.type === 'table') {
          this.viewTableTransactions(targetNode);
        }
        break;
      case 'viewCompactions':
        if (targetNode.type === 'database') {
          this.viewDatabaseCompactions(targetNode);
        }
        break;
      case 'viewLoads':
        if (targetNode.type === 'database') {
          this.viewDatabaseLoads(targetNode);
        }
        break;
      case 'viewDatabaseStats':
        if (targetNode.type === 'database') {
          this.viewDatabaseStats(targetNode);
        }
        break;
      case 'viewTableStats':
        if (targetNode.type === 'table') {
          // Views may not have accurate stats, but we allow viewing
          this.viewTableStats(targetNode);
        }
        break;
      case 'viewCompactionScore':
        if (targetNode.type === 'table') {
          // Views don't have compaction score
          if (targetNode.data?.tableType === 'VIEW') {
            this.toastrService.warning('视图没有Compaction Score信息', '提示');
            return;
          }
          this.viewTableCompactionScore(targetNode);
        }
        break;
      case 'triggerCompaction':
        if (targetNode.type === 'table') {
          // Only regular tables can trigger compaction manually
          const tableType = targetNode.data?.tableType;
          if (tableType === 'VIEW') {
            this.toastrService.warning('视图不支持手动触发Compaction', '提示');
            return;
          }
          if (tableType === 'MATERIALIZED_VIEW') {
            this.toastrService.warning('物化视图的Compaction由系统自动管理，不建议手动触发', '提示');
            return;
          }
          this.openCompactionTriggerDialog(targetNode);
        }
        break;
      default:
        break;
    }
    this.closeContextMenu();
  }

  onContextMenuItemClick(item: TreeContextMenuItem, event: MouseEvent): void {
    event.preventDefault();
    event.stopPropagation();
    this.handleContextMenuAction(item);
  }

  closeContextMenu(): void {
    this.contextMenuVisible = false;
    this.contextMenuItems = [];
    this.contextMenuTargetNode = null;
  }

  private viewTableSchema(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || '';
    const tableName = node.data?.table || node.name;

    if (!databaseName || !tableName) {
      this.toastrService.warning('无法识别该表所属的数据库', '提示');
      return;
    }

    this.schemaDialogTitle = '表结构';
    this.schemaDialogSubtitle = tableName;
    this.currentSchemaCatalog = catalogName || null;
    this.currentSchemaDatabase = databaseName;
    this.currentSchemaTable = tableName;
    this.currentTableSchema = '';
    this.tableSchemaLoading = true;

    const qualifiedTableName = this.buildQualifiedTableName(catalogName, databaseName, tableName);

    if (this.schemaDialogRef) {
      this.schemaDialogRef.close();
    }

    this.schemaDialogRef = this.dialogService.open(this.tableSchemaDialogTemplate, {
      hasBackdrop: true,
      closeOnBackdropClick: true,
      closeOnEsc: true,
    });

    if (this.schemaDialogRef) {
      this.schemaDialogRef.onClose.subscribe(() => {
        this.schemaDialogRef = null;
      });
    }

    const sql = `SHOW CREATE TABLE ${qualifiedTableName}`;

    this.nodeService
      .executeSQL(sql)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (result) => this.handleTableSchemaSuccess(result, tableName),
        error: (error) => {
          this.tableSchemaLoading = false;
          this.currentTableSchema = '';
          this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '获取表结构失败');
        },
      });
  }

  private handleTableSchemaSuccess(result: QueryExecuteResult | null | undefined, tableName: string): void {
    this.tableSchemaLoading = false;

    if (!result || !Array.isArray(result.results) || result.results.length === 0) {
      this.currentTableSchema = '';
      this.toastrService.warning('未返回表结构信息', '提示');
      return;
    }

    const primaryResult = result.results[0];

    if (!primaryResult.success) {
      const errorMessage = primaryResult.error || '执行 SHOW CREATE TABLE 失败';
      this.currentTableSchema = '';
      this.toastrService.danger(errorMessage, '获取表结构失败');
      return;
    }

    const columns = primaryResult.columns || [];
    const rows = primaryResult.rows || [];

    if (!rows || rows.length === 0) {
      this.currentTableSchema = '';
      this.toastrService.warning('未获取到建表语句', '提示');
      return;
    }

    const createIndex = columns.findIndex((column) => (column || '').toLowerCase() === 'create table');
    const tableIndex = columns.findIndex((column) => (column || '').toLowerCase() === 'table');

    const matchedRow = rows.find((row) => {
      if (tableIndex === -1) {
        return true;
      }
      return Array.isArray(row) && row[tableIndex] === tableName;
    }) || rows[0];

    if (!matchedRow) {
      this.currentTableSchema = '';
      this.toastrService.warning('未获取到建表语句', '提示');
      return;
    }

    let createStatement = '';
    if (createIndex !== -1 && matchedRow.length > createIndex) {
      createStatement = matchedRow[createIndex];
    } else if (matchedRow.length > 1) {
      createStatement = matchedRow[1];
    } else if (matchedRow.length > 0) {
      createStatement = matchedRow[0];
    }

    this.currentTableSchema = createStatement || '';
  }

  private buildQualifiedTableName(catalog: string, database: string, table: string): string {
    const parts: string[] = [];

    if (catalog && catalog.trim().length > 0) {
      parts.push(`\`${catalog}\``);
    }

    parts.push(`\`${database}\``);
    parts.push(`\`${table}\``);

    return parts.join('.');
  }

  // Database level view methods
  
  /**
   * Get database ID from cache or query SHOW PROC '/dbs'
   * Returns Observable that emits the database ID or null if not found
   */
  private getDatabaseId(catalogName: string, databaseName: string, node: NavTreeNode): Observable<string | null> {
    // Try to get database ID from cached node data or cache
    let dbId: string | null = node.data?.dbId || null;
    
    if (!dbId) {
      // Try to get from cache
      const dbIdCacheKey = `${catalogName}|${databaseName}`;
      dbId = this.databaseIdCache[dbIdCacheKey] || null;
    }

    if (dbId) {
      // Return cached ID immediately
      return of(dbId);
    }

    // Fallback: query SHOW PROC '/dbs' if not cached
    const getDbIdSql = `SHOW PROC '/dbs'`;
    return this.nodeService.executeSQL(getDbIdSql, 1000, catalogName || undefined, undefined)
      .pipe(
        map((result) => {
          if (result.results && result.results.length > 0 && result.results[0].success) {
            const firstResult = result.results[0];
            let foundDbId: string | null = null;
            
            // Find database ID by matching DbName
            for (const row of firstResult.rows) {
              const dbNameIdx = firstResult.columns.findIndex(col => col === 'DbName');
              const dbIdIdx = firstResult.columns.findIndex(col => col === 'DbId');
              if (dbNameIdx >= 0 && dbIdIdx >= 0 && row[dbNameIdx] === databaseName) {
                foundDbId = String(row[dbIdIdx]);
                // Cache it
                const dbIdCacheKey = `${catalogName}|${databaseName}`;
                this.databaseIdCache[dbIdCacheKey] = foundDbId;
                // Update node data
                if (node.data) {
                  node.data.dbId = foundDbId;
                }
                break;
              }
            }

            return foundDbId;
          }
          return null;
        }),
        catchError((error) => {
          this.toastrService.danger(`获取数据库ID失败: ${ErrorHandler.extractErrorMessage(error)}`, '错误');
          return of(null);
        })
      );
  }

  private viewDatabaseTransactions(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || node.name;

    if (!databaseName) {
      this.toastrService.warning('无法识别数据库名称', '提示');
      return;
    }

    this.getDatabaseId(catalogName, databaseName, node)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (dbId) => {
          if (!dbId) {
            this.toastrService.warning(`无法找到数据库 ${databaseName} 的ID`, '提示');
            return;
          }
          // Open dialog with tab support for running and finished transactions
          this.openTransactionsDialogWithTabs(databaseName, dbId, catalogName);
        },
      });
  }

  private openTransactionsDialogWithTabs(databaseName: string, dbId: string, catalogName?: string, tableName?: string): void {
    // Show loading state immediately
    this.infoDialogTitle = '事务信息';
    this.infoDialogType = 'transactions';
    this.infoDialogPageLoading = true;
    this.infoDialogError = null;
    this.infoDialogData = [];
    this.infoDialogSource.load([]);

    // Define columns for transaction display
    const transactionColumns = {
      TransactionId: { title: '事务ID', type: 'string', width: '12%' },
      Label: { 
        title: '标签', 
        type: 'html', 
        width: '20%',
        valuePrepareFunction: (value: any) => this.renderLongText(value, 30),
      },
      Coordinator: { 
        title: '协调者', 
        type: 'html', 
        width: '15%',
        valuePrepareFunction: (value: any) => this.renderLongText(value, 25),
      },
      TransactionStatus: { 
        title: '状态', 
        type: 'html', 
        width: '10%',
        valuePrepareFunction: (value: string) => {
          const status = value || '';
          if (status === 'VISIBLE') {
            return '<span class="badge badge-success">VISIBLE</span>';
          } else if (status === 'ABORTED') {
            return '<span class="badge badge-danger">ABORTED</span>';
          } else if (status === 'COMMITTED') {
            return '<span class="badge badge-info">COMMITTED</span>';
          }
          return `<span class="badge badge-warning">${status}</span>`;
        },
      },
      LoadJobSourceType: { 
        title: '来源类型', 
        type: 'html', 
        width: '12%',
        valuePrepareFunction: (value: any) => this.renderLongText(value, 20),
      },
      PrepareTime: { title: '准备时间', type: 'string', width: '12%' },
      CommitTime: { 
        title: '提交时间', 
        type: 'html', 
        width: '12%',
        valuePrepareFunction: (value: any) => {
          if (!value || value === 'NULL') {
            return '<span class="badge badge-warning">未提交</span>';
          }
          return String(value);
        },
      },
      PublishTime: { title: '发布时间', type: 'string', width: '12%' },
      FinishTime: { 
        title: '完成时间', 
        type: 'html', 
        width: '12%',
        valuePrepareFunction: (value: any) => {
          if (!value || value === 'NULL') {
            return '<span class="badge badge-info">进行中</span>';
          }
          return String(value);
        },
      },
      ErrMsg: { 
        title: '错误信息', 
        type: 'html', 
        width: '15%',
        valuePrepareFunction: (value: any) => this.renderLongText(value, 30),
      },
    };

    // Set settings BEFORE opening dialog to ensure actions are disabled
    this.infoDialogSettings = {
      mode: 'external',
      hideSubHeader: false,
      noDataMessage: '暂无数据',
      actions: {
        add: false,
        edit: false,
        delete: false,
        position: 'left',
      },
      pager: {
        display: true,
        perPage: this.infoDialogPerPage,
      },
      columns: transactionColumns,
    };
    this.transactionColumns = transactionColumns;

    // Close existing dialog if any
    if (this.infoDialogRef) {
      this.infoDialogRef.close();
    }

    // Open dialog with loading state
    this.infoDialogRef = this.dialogService.open(this.infoDialogTemplate, {
      hasBackdrop: true,
      closeOnBackdropClick: true,
      closeOnEsc: true,
      context: {
        catalog: catalogName,
        database: databaseName,
      },
    });

    // Query running transactions
    const runningSql = `SHOW PROC '/transactions/${dbId}/running'`;
    const finishedSql = `SHOW PROC '/transactions/${dbId}/finished'`;

    // Load both queries
    forkJoin({
      running: this.nodeService.executeSQL(runningSql, 1000, catalogName || undefined, databaseName),
      finished: this.nodeService.executeSQL(finishedSql, 1000, catalogName || undefined, databaseName),
    })
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (results) => {
          // Process running transactions
          let runningData: any[] = [];
          if (results.running.results && results.running.results.length > 0 && results.running.results[0].success) {
            const runningResult = results.running.results[0];
            runningData = runningResult.rows.map(row => {
              const obj: any = {};
              runningResult.columns.forEach((col, idx) => {
                obj[col] = row[idx];
              });
              return obj;
            });
          }

          // Process finished transactions
          let finishedData: any[] = [];
          if (results.finished.results && results.finished.results.length > 0 && results.finished.results[0].success) {
            const finishedResult = results.finished.results[0];
            finishedData = finishedResult.rows.map(row => {
              const obj: any = {};
              finishedResult.columns.forEach((col, idx) => {
                obj[col] = row[idx];
              });
              return obj;
            });
          }

          // Filter by table name if provided (filter by Label field which may contain table name)
          if (tableName) {
            runningData = runningData.filter(item => {
              const label = String(item.Label || '').toLowerCase();
              return label.includes(tableName.toLowerCase());
            });
            finishedData = finishedData.filter(item => {
              const label = String(item.Label || '').toLowerCase();
              return label.includes(tableName.toLowerCase());
            });
          }

          // Store data for tab switching
          this.transactionRunningData = runningData;
          this.transactionFinishedData = finishedData;
          this.transactionCurrentTab = 'running';
          this.transactionColumns = transactionColumns;

          // Update dialog with data
          this.infoDialogSettings = {
            mode: 'external',
            hideSubHeader: false,
            noDataMessage: '暂无数据',
            actions: {
              add: false,
              edit: false,
              delete: false,
              position: 'left',
            },
            pager: {
              display: true,
              perPage: this.infoDialogPerPage,
            },
            columns: transactionColumns,
          };
          this.infoDialogData = runningData;
          this.infoDialogSource.load(runningData);
          this.infoDialogError = null;
          this.infoDialogPageLoading = false;

          if (this.infoDialogRef) {
            this.infoDialogRef.onClose.subscribe(() => {
              this.infoDialogRef = null;
            });
            
            // Wait for table to render, then ensure tooltips work
            setTimeout(() => {
              this.ensureTooltipsWork();
            }, 300);
          }
        },
        error: (error) => {
          this.infoDialogPageLoading = false;
          const errorMessage = ErrorHandler.extractErrorMessage(error);
          this.toastrService.danger(errorMessage, '查询失败');
        },
      });
  }

  private viewDatabaseCompactions(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || node.name;

    if (!databaseName) {
      this.toastrService.warning('无法识别数据库名称', '提示');
      return;
    }

    // Show Compaction tasks using SHOW PROC '/compactions'
    this.openInfoDialog('Compaction任务', 'compactionDetails', () => {
      const sql = `SHOW PROC '/compactions'`;
      return this.nodeService.executeSQL(sql, 1000, catalogName || undefined, databaseName);
    }, {
      columns: {
        Partition: { 
          title: '分区', 
          type: 'html', 
          width: '25%',
          valuePrepareFunction: (value: any) => {
            // Partition format: database.table.partition_id
            const partitionStr = String(value || '');
            const parts = partitionStr.split('.');
            if (parts.length >= 3) {
              const dbName = parts[0];
              const tableName = parts[1];
              const partitionId = parts[2];
              const dbTable = `${dbName}.${tableName}`;
              return `${this.renderLongText(dbTable, 20)}<br><small>分区ID: ${this.renderLongText(partitionId, 15)}</small>`;
            }
            return this.renderLongText(partitionStr, 30);
          },
        },
        TxnID: { title: '事务ID', type: 'string', width: '10%' },
        StartTime: { title: '开始时间', type: 'string', width: '12%' },
        CommitTime: { 
          title: '提交时间', 
          type: 'html', 
          width: '12%',
          valuePrepareFunction: (value: any) => {
            if (!value || value === 'NULL') {
              return '<span class="badge badge-warning">未提交</span>';
            }
            return String(value);
          },
        },
        FinishTime: { 
          title: '完成时间', 
          type: 'html', 
          width: '12%',
          valuePrepareFunction: (value: any) => {
            if (!value || value === 'NULL') {
              return '<span class="badge badge-info">进行中</span>';
            }
            return String(value);
          },
        },
        Error: { 
          title: '错误', 
          type: 'html', 
          width: '15%',
          valuePrepareFunction: (value: any) => this.renderLongText(value, 30),
        },
        Profile: { 
          title: 'Profile', 
          type: 'html', 
          width: '14%',
          valuePrepareFunction: (value: any) => {
            if (!value || value === 'NULL') {
              return '-';
            }
            try {
              const profile = typeof value === 'string' ? JSON.parse(value) : value;
              const subTaskCount = profile.sub_task_count || 0;
              const readLocalMb = profile.read_local_mb || 0;
              const readRemoteMb = profile.read_remote_mb || 0;
              return `<small>子任务: ${subTaskCount}<br>读取: ${readLocalMb}MB本地, ${readRemoteMb}MB远程</small>`;
            } catch {
              return this.renderLongText(value, 20);
            }
          },
        },
      },
    }, catalogName, databaseName);
  }

  private viewDatabaseLoads(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || node.name;

    if (!databaseName) {
      this.toastrService.warning('无法识别数据库名称', '提示');
      return;
    }

    this.openInfoDialog('导入作业', 'loads', () => {
      const sql = `SELECT 
        JOB_ID,
        LABEL,
        STATE,
        PROGRESS,
        TYPE,
        PRIORITY,
        SCAN_ROWS,
        FILTERED_ROWS,
        SINK_ROWS,
        CREATE_TIME,
        LOAD_START_TIME,
        LOAD_FINISH_TIME,
        ERROR_MSG
      FROM information_schema.loads 
      WHERE DB_NAME = '${databaseName}'
      ORDER BY CREATE_TIME DESC
      LIMIT 100`;

      return this.nodeService.executeSQL(sql, 100, catalogName || undefined, databaseName);
    }, {
      columns: {
        JOB_ID: { title: '作业ID', type: 'string', width: '10%' },
        LABEL: { 
          title: '标签', 
          type: 'html', 
          width: '15%',
          valuePrepareFunction: (value: any) => this.renderLongText(value, 30),
        },
        STATE: { 
          title: '状态', 
          type: 'html', 
          width: '10%',
          valuePrepareFunction: (value: string) => this.renderLoadState(value),
        },
        PROGRESS: { title: '进度', type: 'string', width: '12%' },
        TYPE: { title: '类型', type: 'string', width: '8%' },
        PRIORITY: { title: '优先级', type: 'string', width: '8%' },
        SCAN_ROWS: { title: '扫描行数', type: 'string', width: '10%' },
        SINK_ROWS: { title: '导入行数', type: 'string', width: '10%' },
        CREATE_TIME: { title: '创建时间', type: 'string', width: '12%' },
        ERROR_MSG: { 
          title: '错误信息', 
          type: 'html', 
          width: '5%',
          valuePrepareFunction: (value: any) => this.renderLongText(value, 30),
        },
      },
    }, catalogName, databaseName);
  }

  private viewDatabaseStats(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || node.name;

    if (!databaseName) {
      this.toastrService.warning('无法识别数据库名称', '提示');
      return;
    }

    this.openInfoDialog('数据库统计', 'databaseStats', () => {
      const sql = `SELECT 
        TABLE_NAME,
        COUNT(DISTINCT PARTITION_NAME) as PARTITION_COUNT,
        SUM(ROW_COUNT) as TOTAL_ROWS,
        ROUND(SUM(CASE 
                 WHEN DATA_SIZE LIKE '%KB' THEN CAST(REPLACE(DATA_SIZE, 'KB', '') AS DECIMAL) / 1024
                 WHEN DATA_SIZE LIKE '%MB' THEN CAST(REPLACE(DATA_SIZE, 'MB', '') AS DECIMAL)
                 WHEN DATA_SIZE LIKE '%GB' THEN CAST(REPLACE(DATA_SIZE, 'GB', '') AS DECIMAL) * 1024
                 WHEN DATA_SIZE LIKE '%TB' THEN CAST(REPLACE(DATA_SIZE, 'TB', '') AS DECIMAL) * 1024 * 1024
                 WHEN DATA_SIZE LIKE '%B' AND DATA_SIZE != '0B' THEN CAST(REPLACE(REPLACE(DATA_SIZE, 'B', ''), ' ', '') AS DECIMAL) / 1024 / 1024
                 ELSE 0 
             END), 2) as TOTAL_SIZE_MB,
        ROUND(AVG(MAX_CS), 2) as AVG_MAX_CS,
        MAX(MAX_CS) as MAX_CS_OVERALL
      FROM information_schema.partitions_meta 
      WHERE DB_NAME = '${databaseName}'
      GROUP BY TABLE_NAME
      ORDER BY TOTAL_ROWS DESC`;

      return this.nodeService.executeSQL(sql, 100, catalogName || undefined, databaseName);
    }, {
      columns: {
        TABLE_NAME: { title: '表名', type: 'string', width: '20%' },
        PARTITION_COUNT: { title: '分区数', type: 'string', width: '12%' },
        TOTAL_ROWS: { title: '总行数', type: 'string', width: '15%' },
        TOTAL_SIZE_MB: { 
          title: '总大小(MB)', 
          type: 'html', 
          width: '15%',
          valuePrepareFunction: (value: any) => {
            if (value === null || value === undefined || value === '') {
              return '0.00';
            }
            const num = typeof value === 'string' ? parseFloat(value) : value;
            return isNaN(num) ? '0.00' : num.toFixed(2);
          },
        },
        AVG_MAX_CS: { 
          title: '平均最大CS', 
          type: 'html', 
          width: '15%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        MAX_CS_OVERALL: { 
          title: '最大CS', 
          type: 'html', 
          width: '15%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
      },
    }, catalogName, databaseName);
  }

  // Table level view methods
  private viewTablePartitions(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || '';
    const tableName = node.data?.table || node.name;

    if (!databaseName || !tableName) {
      this.toastrService.warning('无法识别该表所属的数据库', '提示');
      return;
    }

    // Views don't have partitions
    if (node.data?.tableType === 'VIEW') {
      this.toastrService.warning('视图是逻辑表，没有物理分区信息', '提示');
      return;
    }

    this.openInfoDialog('分区信息', 'partitions', () => {
      const sql = `SELECT 
        PARTITION_NAME,
        PARTITION_ID,
        PARTITION_KEY,
        PARTITION_VALUE,
        DATA_SIZE,
        ROW_COUNT,
        AVG_CS,
        P50_CS,
        MAX_CS,
        COMPACT_VERSION,
        VISIBLE_VERSION,
        STORAGE_PATH
      FROM information_schema.partitions_meta 
      WHERE DB_NAME = '${databaseName}' AND TABLE_NAME = '${tableName}'
      ORDER BY PARTITION_NAME`;

      return this.nodeService.executeSQL(sql, 100, catalogName || undefined, databaseName);
    }, {
      columns: {
        PARTITION_NAME: { title: '分区名', type: 'string', width: '15%' },
        PARTITION_ID: { title: '分区ID', type: 'string', width: '10%' },
        PARTITION_KEY: { 
          title: '分区键', 
          type: 'html', 
          width: '12%',
          valuePrepareFunction: (value: any) => this.renderLongText(value, 40),
        },
        PARTITION_VALUE: { 
          title: '分区值', 
          type: 'html', 
          width: '12%',
          valuePrepareFunction: (value: any) => this.renderLongText(value, 40),
        },
        DATA_SIZE: { title: '数据大小', type: 'string', width: '10%' },
        ROW_COUNT: { title: '行数', type: 'string', width: '10%' },
        AVG_CS: { 
          title: '平均CS', 
          type: 'html', 
          width: '8%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        MAX_CS: { 
          title: '最大CS', 
          type: 'html', 
          width: '8%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        COMPACT_VERSION: { title: 'Compact版本', type: 'string', width: '10%' },
        VISIBLE_VERSION: { title: '可见版本', type: 'string', width: '10%' },
        STORAGE_PATH: { 
          title: '存储路径', 
          type: 'html', 
          width: '7%',
          valuePrepareFunction: (value: any) => this.renderLongText(value, 30),
        },
      },
    }, catalogName, databaseName);
  }

  private viewTableCompactionScore(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || '';
    const tableName = node.data?.table || node.name;

    if (!databaseName || !tableName) {
      this.toastrService.warning('无法识别该表所属的数据库', '提示');
      return;
    }

    // Views don't have compaction score
    if (node.data?.tableType === 'VIEW') {
      this.toastrService.warning('视图是逻辑表，没有Compaction Score信息', '提示');
      return;
    }

    this.openInfoDialog('Compaction Score', 'compactionScore', () => {
      const sql = `SELECT 
        PARTITION_NAME,
        AVG_CS,
        P50_CS,
        MAX_CS,
        DATA_SIZE,
        ROW_COUNT,
        COMPACT_VERSION,
        VISIBLE_VERSION
      FROM information_schema.partitions_meta 
      WHERE DB_NAME = '${databaseName}' AND TABLE_NAME = '${tableName}'
      ORDER BY MAX_CS DESC`;

      return this.nodeService.executeSQL(sql, 100, catalogName || undefined, databaseName);
    }, {
      columns: {
        PARTITION_NAME: { title: '分区名', type: 'string', width: '15%' },
        AVG_CS: { 
          title: '平均CS', 
          type: 'html', 
          width: '12%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        P50_CS: { 
          title: 'P50 CS', 
          type: 'html', 
          width: '12%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        MAX_CS: { 
          title: '最大CS', 
          type: 'html', 
          width: '12%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        DATA_SIZE: { title: '数据大小', type: 'string', width: '12%' },
        ROW_COUNT: { title: '行数', type: 'string', width: '12%' },
        COMPACT_VERSION: { title: 'Compact版本', type: 'string', width: '12%' },
        VISIBLE_VERSION: { title: '可见版本', type: 'string', width: '13%' },
      },
    }, catalogName, databaseName);
  }

  private viewTableStats(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || '';
    const tableName = node.data?.table || node.name;

    if (!databaseName || !tableName) {
      this.toastrService.warning('无法识别该表所属的数据库', '提示');
      return;
    }

    // Views may not have accurate partition stats, but we allow viewing
    // Materialized views have physical storage, so they have stats
    // Regular tables have stats
    // So we only block pure views from partition-based stats
    // Note: We allow viewing even for views, as they might have some metadata

    this.openInfoDialog('表统计', 'tableStats', () => {
      const sql = `SELECT 
        PARTITION_NAME,
        PARTITION_ID,
        DATA_SIZE,
        ROW_COUNT,
        BUCKETS,
        REPLICATION_NUM,
        STORAGE_MEDIUM,
        AVG_CS,
        MAX_CS,
        STORAGE_PATH
      FROM information_schema.partitions_meta 
      WHERE DB_NAME = '${databaseName}' AND TABLE_NAME = '${tableName}'
      ORDER BY PARTITION_NAME`;

      return this.nodeService.executeSQL(sql, 100, catalogName || undefined, databaseName);
    }, {
      columns: {
        PARTITION_NAME: { title: '分区名', type: 'string', width: '15%' },
        PARTITION_ID: { title: '分区ID', type: 'string', width: '10%' },
        DATA_SIZE: { title: '数据大小', type: 'string', width: '12%' },
        ROW_COUNT: { title: '行数', type: 'string', width: '12%' },
        BUCKETS: { title: '分桶数', type: 'string', width: '8%' },
        REPLICATION_NUM: { title: '副本数', type: 'string', width: '8%' },
        STORAGE_MEDIUM: { title: '存储介质', type: 'string', width: '10%' },
        AVG_CS: { 
          title: '平均CS', 
          type: 'html', 
          width: '8%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        MAX_CS: { 
          title: '最大CS', 
          type: 'html', 
          width: '8%',
          valuePrepareFunction: (value: number) => this.renderCompactionScore(value),
        },
        STORAGE_PATH: { 
          title: '存储路径', 
          type: 'html', 
          width: '9%',
          valuePrepareFunction: (value: any) => this.renderLongText(value, 30),
        },
      },
    }, catalogName, databaseName);
  }

  private viewTableTransactions(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || '';
    const tableName = node.data?.table || node.name;

    if (!databaseName || !tableName) {
      this.toastrService.warning('无法识别该表所属的数据库', '提示');
      return;
    }

    // Views don't have transactions (they are logical, not physical)
    if (node.data?.tableType === 'VIEW') {
      this.toastrService.warning('视图是逻辑表，不涉及物理事务', '提示');
      return;
    }

    this.getDatabaseId(catalogName, databaseName, node)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (dbId) => {
          if (!dbId) {
            this.toastrService.warning(`无法找到数据库 ${databaseName} 的ID`, '提示');
            return;
          }
          // Open dialog with tab support, filtering by table name
          this.openTransactionsDialogWithTabs(databaseName, dbId, catalogName, tableName);
        },
      });
  }

  switchTransactionTab(tab: 'running' | 'finished'): void {
    this.transactionCurrentTab = tab;
    const data = tab === 'running' ? this.transactionRunningData : this.transactionFinishedData;
    this.infoDialogData = data;
    this.infoDialogSource.load(data);
    
    // Wait for table to render after tab switch, then ensure tooltips work
    setTimeout(() => {
      this.ensureTooltipsWork();
    }, 300);
  }

  // Helper methods for info dialog
  private openInfoDialog(
    title: string,
    type: 'transactions' | 'compactions' | 'compactionDetails' | 'loads' | 'databaseStats' | 'tableStats' | 'partitions' | 'compactionScore',
    queryFn: () => Observable<QueryExecuteResult>,
    settings: any,
    catalog?: string,
    database?: string
  ): void {
    this.infoDialogTitle = title;
    this.infoDialogType = type;
    // Load perPage preference from localStorage
    const savedPerPage = localStorage.getItem('infoDialogPerPage');
    if (savedPerPage) {
      const parsed = parseInt(savedPerPage, 10);
      if (this.perPageOptions.includes(parsed)) {
        this.infoDialogPerPage = parsed;
      }
    }
    
    this.infoDialogSettings = {
      mode: 'external',
      hideSubHeader: false,
      noDataMessage: '暂无数据',
      actions: {
        add: false,
        edit: false,
        delete: false,
        position: 'left',
      },
      pager: {
        display: true,
        perPage: this.infoDialogPerPage,
      },
      columns: settings.columns,
    };
    this.infoDialogLoading = false; // Don't show loading in dialog
    this.infoDialogError = null;
    this.infoDialogData = [];
    this.infoDialogSource.load([]);
    this.infoDialogPageLoading = true; // Show page-level loading

    // Close existing dialog if any
    if (this.infoDialogRef) {
      this.infoDialogRef.close();
    }

    // Open dialog immediately with loading state
    this.infoDialogRef = this.dialogService.open(this.infoDialogTemplate, {
      hasBackdrop: true,
      closeOnBackdropClick: true,
      closeOnEsc: true,
      context: {
        catalog,
        database,
      },
    });

    if (this.infoDialogRef) {
      this.infoDialogRef.onClose.subscribe(() => {
        this.infoDialogRef = null;
      });
    }

    // Load data
    queryFn()
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (result) => {
          this.infoDialogPageLoading = false; // Hide loading

          if (result.results && result.results.length > 0 && result.results[0].success) {
            const firstResult = result.results[0];
            let dataRows = firstResult.rows.map(row => {
              const obj: any = {};
              firstResult.columns.forEach((col, idx) => {
                obj[col] = row[idx];
              });
              return obj;
            });

            // Filter Compaction tasks by database if database is provided
            if (type === 'compactionDetails' && database) {
              dataRows = dataRows.filter((row: any) => {
                const partition = String(row.Partition || '');
                // Partition format: database.table.partition_id
                return partition.startsWith(`${database}.`);
              });
            }

            this.infoDialogData = dataRows;
            this.infoDialogSource.load(dataRows);
            this.infoDialogError = null;

              // Wait for table to render, then ensure tooltips work
              setTimeout(() => {
                this.ensureTooltipsWork();
              }, 300);
          } else {
            const error = result.results?.[0]?.error || '查询失败';
            this.infoDialogError = error;
            this.infoDialogData = [];
            this.infoDialogSource.load([]);
            
            // Show error (dialog already open)
            this.toastrService.danger(error, '查询失败');
          }
        },
        error: (error) => {
          this.infoDialogPageLoading = false; // Hide loading
          const errorMessage = ErrorHandler.extractErrorMessage(error);
          this.infoDialogError = errorMessage;
          this.infoDialogData = [];
          this.infoDialogSource.load([]);
          
          // Show error (dialog already open)
          this.toastrService.danger(errorMessage, '查询失败');
        },
      });
  }

  // Helper method to render long text with truncation and tooltip
  // Now uses the shared utility function
  private renderLongText(value: any, maxLength: number = 50): string {
    return renderLongText(value, maxLength);
  }

  // Ensure tooltips work in ng2-smart-table and add copy functionality
  // This is a workaround for cases where ng2-smart-table doesn't properly render title attributes
  private ensureTooltipsWork(): void {
    if (!this.infoDialogRef) return;
    
    // Find all spans with title attributes in the dialog
    const dialogElement = document.querySelector('.info-dialog-card');
    if (!dialogElement) return;
    
    const spansWithTitle = dialogElement.querySelectorAll('span[title]');
    spansWithTitle.forEach((span: Element) => {
      const title = span.getAttribute('title');
      if (title && span.textContent) {
        // Ensure the title attribute is set (in case it was stripped)
        span.setAttribute('title', title);
        // Add cursor style if not already present
        if (!span.getAttribute('style') || !span.getAttribute('style')?.includes('cursor')) {
          const currentStyle = span.getAttribute('style') || '';
          span.setAttribute('style', currentStyle + (currentStyle ? '; ' : '') + 'cursor: help;');
        }
        
        // Add click to copy functionality (right-click or double-click)
        span.addEventListener('dblclick', (e) => {
          e.stopPropagation();
          this.copyToClipboard(title);
        });
        
        // Also support right-click context menu for copy
        span.addEventListener('contextmenu', (e) => {
          e.preventDefault();
          e.stopPropagation();
          this.copyToClipboard(title);
        });
        
        // Add visual indicator
        span.setAttribute('data-copyable', 'true');
        const originalTitle = span.getAttribute('title') || title;
        span.setAttribute('title', originalTitle + ' (双击或右键复制)');
      }
    });
  }

  private copyToClipboard(text: string): void {
    if (!text) return;

    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(() => {
        this.toastrService.success('已复制到剪贴板', '成功', { duration: 2000 });
      }).catch((err) => {
        console.error('Failed to copy text:', err);
        this.fallbackCopyText(text);
      });
    } else {
      this.fallbackCopyText(text);
    }
  }

  // Handle per page change
  onPerPageChange(newPerPage: number): void {
    this.infoDialogPerPage = newPerPage;
    localStorage.setItem('infoDialogPerPage', newPerPage.toString());
    
    // Update settings and reload data
    this.infoDialogSettings = {
      ...this.infoDialogSettings,
      pager: {
        ...this.infoDialogSettings.pager,
        perPage: newPerPage,
      },
    };
    
    // Reload data source to apply new pagination
    this.infoDialogSource.setPaging(1, newPerPage, true);
  }

  private fallbackCopyText(text: string): void {
    const textArea = document.createElement('textarea');
    textArea.value = text;
    textArea.style.position = 'fixed';
    textArea.style.left = '-999999px';
    textArea.style.top = '-999999px';
    document.body.appendChild(textArea);
    textArea.focus();
    textArea.select();
    
    try {
      document.execCommand('copy');
      this.toastrService.success('已复制到剪贴板', '成功', { duration: 2000 });
    } catch (err) {
      console.error('Fallback copy failed:', err);
      this.toastrService.danger('复制失败', '错误');
    }
    
    document.body.removeChild(textArea);
  }

  // Render helper methods
  private renderCompactionScore(value: number): string {
    if (value === null || value === undefined) {
      return '<span class="badge badge-basic">-</span>';
    }
    const numValue = typeof value === 'string' ? parseFloat(value) : value;
    if (isNaN(numValue)) {
      return '<span class="badge badge-basic">-</span>';
    }
    
    let badgeClass = 'badge-success'; // < 10: green
    if (numValue >= 2000) {
      badgeClass = 'badge-danger'; // >= 2000: red
    } else if (numValue >= 100) {
      badgeClass = 'badge-warning'; // >= 100: orange/yellow
    } else if (numValue >= 10) {
      badgeClass = 'badge-info'; // >= 10: yellow
    }
    
    return `<span class="badge ${badgeClass}">${numValue.toFixed(2)}</span>`;
  }

  private renderLoadState(value: string): string {
    if (!value) return '-';
    const badges: { [key: string]: string } = {
      FINISHED: '<span class="badge badge-success">完成</span>',
      LOADING: '<span class="badge badge-info">加载中</span>',
      PENDING: '<span class="badge badge-warning">等待中</span>',
      CANCELLED: '<span class="badge badge-danger">已取消</span>',
      QUEUEING: '<span class="badge badge-info">队列中</span>',
    };
    return badges[value] || `<span class="badge badge-basic">${value}</span>`;
  }

  // Compaction trigger dialog
  private openCompactionTriggerDialog(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const databaseName = node.data?.database || '';
    const tableName = node.data?.table || node.name;

    if (!databaseName || !tableName) {
      this.toastrService.warning('无法识别该表所属的数据库', '提示');
      return;
    }

    // Only regular tables can trigger compaction manually
    const tableType = node.data?.tableType;
    if (tableType === 'VIEW') {
      this.toastrService.warning('视图不支持手动触发Compaction', '提示');
      return;
    }
    if (tableType === 'MATERIALIZED_VIEW') {
      this.toastrService.warning('物化视图的Compaction由系统自动管理，不建议手动触发', '提示');
      return;
    }

    this.compactionTriggerTable = tableName;
    this.compactionTriggerDatabase = databaseName;
    this.compactionTriggerCatalog = catalogName;
    this.compactionSelectedPartitions = [];
    this.compactionTriggerMode = 'table';
    this.compactionTriggering = false;

    // Load partitions for selection
    const sql = `SELECT PARTITION_NAME 
      FROM information_schema.partitions_meta 
      WHERE DB_NAME = '${databaseName}' AND TABLE_NAME = '${tableName}'
      ORDER BY PARTITION_NAME`;

    this.nodeService
      .executeSQL(sql, 100, catalogName || undefined, databaseName)
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (result) => {
          if (result.results && result.results.length > 0 && result.results[0].success) {
            this.availablePartitions = result.results[0].rows.map(row => row[0]);
          }
        },
        error: (error) => {
          console.error('Failed to load partitions:', error);
        },
      });

    if (this.compactionTriggerDialogRef) {
      this.compactionTriggerDialogRef.close();
    }

    this.compactionTriggerDialogRef = this.dialogService.open(this.compactionTriggerDialogTemplate, {
      hasBackdrop: true,
      closeOnBackdropClick: true,
      closeOnEsc: true,
    });

    if (this.compactionTriggerDialogRef) {
      this.compactionTriggerDialogRef.onClose.subscribe(() => {
        this.compactionTriggerDialogRef = null;
      });
    }
  }

  closeCompactionTriggerDialog(): void {
    if (this.compactionTriggerDialogRef) {
      this.compactionTriggerDialogRef.close();
    }
  }

  togglePartitionSelection(partition: string, checked: boolean): void {
    if (checked) {
      if (!this.compactionSelectedPartitions.includes(partition)) {
        this.compactionSelectedPartitions.push(partition);
      }
    } else {
      const index = this.compactionSelectedPartitions.indexOf(partition);
      if (index > -1) {
        this.compactionSelectedPartitions.splice(index, 1);
      }
    }
  }

  triggerCompaction(): void {
    if (!this.compactionTriggerTable || !this.compactionTriggerDatabase) {
      return;
    }

    if (this.compactionTriggerMode === 'partition' && this.compactionSelectedPartitions.length === 0) {
      this.toastrService.warning('请至少选择一个分区', '提示');
      return;
    }

    const qualifiedTableName = this.buildQualifiedTableName(
      this.compactionTriggerCatalog || '',
      this.compactionTriggerDatabase,
      this.compactionTriggerTable
    );

    let actionDesc = '';
    if (this.compactionTriggerMode === 'table') {
      actionDesc = `对整个表 "${this.compactionTriggerTable}" 执行Compaction`;
    } else {
      if (this.compactionSelectedPartitions.length === 1) {
        actionDesc = `对分区 "${this.compactionSelectedPartitions[0]}" 执行Compaction`;
      } else {
        actionDesc = `对 ${this.compactionSelectedPartitions.length} 个分区执行Compaction`;
      }
    }

    this.confirmDialogService
      .confirm(
        '确认触发Compaction',
        `确定要${actionDesc}吗？\n\nCompaction任务会在后台执行，不会阻塞当前操作。`,
        '确认触发',
        '取消',
        'primary'
      )
      .subscribe((confirmed) => {
        if (!confirmed) {
          return;
        }

        this.compactionTriggering = true;
        let sql = '';
        if (this.compactionTriggerMode === 'table') {
          sql = `ALTER TABLE ${qualifiedTableName} COMPACT`;
        } else {
          if (this.compactionSelectedPartitions.length === 1) {
            sql = `ALTER TABLE ${qualifiedTableName} COMPACT \`${this.compactionSelectedPartitions[0]}\``;
          } else {
            const partitions = this.compactionSelectedPartitions.map(p => `\`${p}\``).join(', ');
            sql = `ALTER TABLE ${qualifiedTableName} COMPACT (${partitions})`;
          }
        }

        this.nodeService
          .executeSQL(sql, undefined, this.compactionTriggerCatalog || undefined, this.compactionTriggerDatabase)
          .pipe(takeUntil(this.destroy$))
          .subscribe({
            next: (result) => {
              this.compactionTriggering = false;
              if (result.results && result.results.length > 0 && result.results[0].success) {
                this.toastrService.success('Compaction任务已触发', '成功');
                this.closeCompactionTriggerDialog();
              } else {
                const error = result.results?.[0]?.error || '触发失败';
                this.toastrService.danger(error, '触发Compaction失败');
              }
            },
            error: (error) => {
              this.compactionTriggering = false;
              this.toastrService.danger(ErrorHandler.extractErrorMessage(error), '触发Compaction失败');
            },
          });
      });
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
    this.closeContextMenu();
    this.refreshSqlSchema();
  }

  private loadDatabasesForCatalog(node: NavTreeNode): void {
    const catalogName = node.data?.catalog || '';
    const cacheKey = this.getCatalogKey(catalogName);

    if (this.databaseCache[cacheKey]) {
      node.children = this.databaseCache[cacheKey].map((db) => {
        const dbNode = this.createDatabaseNode(catalogName, db);
        // Restore cached database ID if available
        const dbIdCacheKey = `${catalogName}|${db}`;
        if (this.databaseIdCache[dbIdCacheKey] && dbNode.data) {
          dbNode.data.dbId = this.databaseIdCache[dbIdCacheKey];
        }
        return dbNode;
      });
      return;
    }

    node.loading = true;
    this.loadingDatabases = true;

    // Load databases and their IDs in parallel
    forkJoin({
      databases: this.nodeService.getDatabases(catalogName || undefined),
      dbIds: this.nodeService.executeSQL(`SHOW PROC '/dbs'`, 1000, catalogName || undefined, undefined),
    })
      .pipe(takeUntil(this.destroy$))
      .subscribe({
        next: (results) => {
          const dbList = results.databases || [];
        this.databaseCache[cacheKey] = dbList;
          
          // Parse database IDs from SHOW PROC '/dbs'
          if (results.dbIds.results && results.dbIds.results.length > 0 && results.dbIds.results[0].success) {
            const dbIdsResult = results.dbIds.results[0];
            const dbNameIdx = dbIdsResult.columns.findIndex(col => col === 'DbName');
            const dbIdIdx = dbIdsResult.columns.findIndex(col => col === 'DbId');
            
            if (dbNameIdx >= 0 && dbIdIdx >= 0) {
              for (const row of dbIdsResult.rows) {
                const dbName = String(row[dbNameIdx] || '');
                const dbId = String(row[dbIdIdx] || '');
                if (dbName && dbId) {
                  const dbIdCacheKey = `${catalogName}|${dbName}`;
                  this.databaseIdCache[dbIdCacheKey] = dbId;
                }
              }
            }
          }
          
          // Create database nodes with cached IDs
          node.children = dbList.map((db) => {
            const dbNode = this.createDatabaseNode(catalogName, db);
            const dbIdCacheKey = `${catalogName}|${db}`;
            if (this.databaseIdCache[dbIdCacheKey] && dbNode.data) {
              dbNode.data.dbId = this.databaseIdCache[dbIdCacheKey];
            }
            return dbNode;
          });
          
        node.loading = false;
        this.loadingDatabases = false;
        this.refreshSqlSchema();
        
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
        this.refreshSqlSchema();
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

    const applyTables = (tables: TableInfo[]) => {
      const tableList = tables ? [...tables] : [];
      this.tableCache[cacheKey] = tableList;
      node.children = tableList.map((table) => this.createTableNode(catalogName, databaseName, table));
      const baseName = node.data?.originalName || databaseName;
      node.name = `${baseName}${tableList.length > 0 ? ` (${tableList.length})` : ''}`;
      if (node.data) {
        node.data.tablesLoaded = true;
        node.data.tableCount = tableList.length;
      }
      if (node.expanded && this.selectedNodeId === node.id && node.children.length > 0) {
        this.onNodeSelect(node.children[0]);
      }
      this.refreshSqlSchema();
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
        this.refreshSqlSchema();
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

    // Destroy existing editor if any
    if (this.editorView) {
      this.editorView.destroy();
      this.editorView = null;
    }

    const extensions: Extension[] = [
      history(),
      drawSelection(),
      highlightActiveLine(),
      highlightActiveLineGutter(),
      highlightSelectionMatches(),
      syntaxHighlighting(defaultHighlightStyle),
      keymap.of([
        ...completionKeymap,
        ...historyKeymap,
        ...closeBracketsKeymap,
        ...searchKeymap,
      ] as any),
      closeBrackets(),
      autocompletion({
        override: [
          (context) => {
            // Build schema-aware completions based on context (for dot notation)
            const schemaResult = this.buildSchemaCompletions(context);
            
            // If we have schema completions (e.g., after a dot), return them
            if (schemaResult && schemaResult.completions.length > 0) {
              return {
                from: schemaResult.from,
                options: schemaResult.completions,
              };
            }
            
            // Check if we're after a dot - if so, return empty to prevent keyword completion
            const textBefore = context.state.doc.sliceString(Math.max(0, context.pos - 50), context.pos);
            const isAfterDot = /[\w.]\.\s*$/.test(textBefore);
            
            if (isAfterDot) {
              // After a dot but no completions found, return empty
              return { from: context.pos, options: [] };
            }
            
            // For keyword completions, manually add SQL keywords
            // This is necessary because override prevents SQL extension's default completions
            const keywordResult = this.buildKeywordCompletions(context);
            if (keywordResult && keywordResult.completions.length > 0) {
              return {
                from: keywordResult.from,
                options: keywordResult.completions,
              };
            }
            
            // Return null to try other completion sources
            return null;
          },
        ],
        activateOnTyping: true,
        defaultKeymap: true,
        maxRenderedOptions: 50,
      }),
      EditorView.updateListener.of((update) => {
        if (update.docChanged) {
          this.sqlInput = update.state.doc.toString();
        }
      }),
      this.themeCompartment.of(this.buildEditorTheme()),
      this.sqlConfigCompartment.of(
        sql({
          dialect: this.sqlDialect,
          upperCaseKeywords: true,
          schema: this.currentSqlSchema,
        }),
      ),
    ];

    const state = EditorState.create({
      doc: this.sqlInput || '',
      extensions,
    });

    this.editorView = new EditorView({
      state,
      parent: this.editorContainer.nativeElement,
    });
  }

  private updateEditorTheme(): void {
    this.applyEditorTheme();
  }

  private destroyEditor(): void {
    if (this.editorView) {
      this.editorView.destroy();
      this.editorView = null;
    }
  }

  private loadCatalogs(autoSelectFirst = true): void {
    // Backend will get active cluster automatically - no need to check clusterId
    this.loadingCatalogs = true;
    this.nodeService.getCatalogs().subscribe({
      next: (catalogs) => {
        const catalogList = (catalogs || []).filter((name) => !!name && name.trim().length > 0);
        catalogList.sort((a, b) => a.localeCompare(b));
        this.catalogs = catalogList;
        this.loadingCatalogs = false;
        this.databaseTree = this.catalogs.map((catalog) => this.createCatalogNode(catalog));
        this.refreshSqlSchema();

        if (autoSelectFirst && this.databaseTree.length > 0) {
          const firstCatalogNode = this.databaseTree[0];
          this.onNodeSelect(firstCatalogNode);
          this.toggleNode(firstCatalogNode);
        }
      },
      error: (error) => {
        this.loadingCatalogs = false;
        console.error('Failed to load catalogs:', error);
        this.refreshSqlSchema();
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
    if (this.schemaDialogRef) {
      this.schemaDialogRef.close();
      this.schemaDialogRef = null;
    }
    this.contextMenuVisible = false;
    this.contextMenuTargetNode = null;
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
      // Stop auto-refresh if user is not authenticated (logged out)
      if (!this.authService.isAuthenticated()) {
        this.autoRefresh = false;
        this.stopAutoRefresh();
        return;
      }
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

    // Recalculate editor dimensions immediately to sync CodeMirror theme height
    this.calculateEditorHeight();

    // When animation completes, recalc again to ensure layout settles
    setTimeout(() => this.calculateEditorHeight(), 220);
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

    const trimmedSql = this.sqlInput.trim();
    if (this.containsDangerousStatement(trimmedSql)) {
      this.confirmDialogService.confirm(
        '危险操作确认',
        '检测到 SQL 包含删除或破坏性语句，是否继续执行？',
        '继续执行',
        '取消',
        'danger',
      ).subscribe((confirmed) => {
        if (!confirmed) {
          return;
        }
        this.executeSQLInternal(trimmedSql);
      });
      return;
    }

    this.executeSQLInternal(trimmedSql);
  }

  private executeSQLInternal(sql: string): void {
    this.executing = true;
    this.queryResult = null;
    this.resultSettings = [];
    this.queryResults = [];
    this.resultSources = [];
    this.currentResultIndex = 0;

    this.nodeService.executeSQL(
      sql,
      this.queryLimit,
      this.selectedCatalog || undefined,
      this.selectedDatabase || undefined,
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
            '成功',
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

  private containsDangerousStatement(sql: string): boolean {
    const normalized = sql
      .replace(/--.*$/gm, '')
      .replace(/#.*/gm, '')
      .replace(/\/\/.*$/gm, '')
      .replace(/\/\*[\s\S]*?\*\//g, '');
    const tokens = normalized
      .split(';')
      .map(segment => segment.trim().toUpperCase())
      .filter(segment => segment.length > 0);

    if (tokens.length === 0) {
      return false;
    }

    const dangerousPrefixes = ['DELETE', 'DROP', 'TRUNCATE', 'ALTER'];

    return tokens.some(statement => dangerousPrefixes.some(prefix => statement.startsWith(prefix)));
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
        perPage: 15,
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

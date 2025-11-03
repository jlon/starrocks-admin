import { Component, OnInit, OnDestroy, ViewChild, AfterViewInit, ElementRef, HostListener, TemplateRef } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { NbDialogRef, NbDialogService, NbToastrService, NbThemeService } from '@nebular/theme';
import { LocalDataSource } from 'ng2-smart-table';
import { Subject } from 'rxjs';
import { takeUntil } from 'rxjs/operators';
import { NodeService, QueryExecuteResult, SingleQueryResult } from '../../../../@core/data/node.service';
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

type NavNodeType = 'catalog' | 'database' | 'group' | 'table';

type ContextMenuAction = 'viewSchema' | 'viewPartitions';

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
  private currentSqlSchema: SQLNamespace = {};
  treePanelHeight: number = 420;
  private readonly treeExtraHeight: number = 140;
  treeCollapsed: boolean = false;
  private previousTreeWidth: number = this.treePanelWidth;
  readonly collapsedTreeWidth: number = 28;
  private readonly sqlDialect = MySQL;
  private readonly themeCompartment = new Compartment();
  private readonly sqlConfigCompartment = new Compartment();

  // Table schema dialog state
  schemaDialogTitle: string = '';
  schemaDialogSubtitle: string = '';
  currentSchemaCatalog: string | null = null;
  currentSchemaDatabase: string | null = null;
  currentSchemaTable: string | null = null;
  currentTableSchema: string = '';
  tableSchemaLoading: boolean = false;
  private schemaDialogRef: NbDialogRef<any> | null = null;
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
    private dialogService: NbDialogService,
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
        dbNamespace[databaseName] = tables.length > 0 ? tables : [];
      });

      namespace[catalogName || 'default'] = dbNamespace;
    });

    if (Object.keys(namespace).length === 0 && Object.keys(this.tableCache).length > 0) {
      const fallback: Record<string, SQLNamespace> = {};
      Object.entries(this.tableCache).forEach(([cacheKey, tables]) => {
        const [, databaseName] = cacheKey.split('|');
        if (databaseName) {
          fallback[databaseName] = tables.length > 0 ? tables : [];
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
    if (node.type === 'table') {
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
        this.toastrService.info('分区信息功能即将上线', '敬请期待');
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

    const applyTables = (tables: string[]) => {
      const tableList = tables || [];
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
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
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

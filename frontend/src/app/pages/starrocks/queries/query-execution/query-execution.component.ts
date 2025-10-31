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
import { autocompletion } from '@codemirror/autocomplete';
import { format } from 'sql-formatter';
import { trigger, transition, style, animate, state } from '@angular/animations';

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
  
  databases: string[] = [];
  selectedDatabase: string | null = null;
  loadingDatabases: boolean = false;

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
      this.loadCatalogs();
      this.calculateEditorHeight();
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
  
  // Calculate dynamic editor height based on viewport
  calculateEditorHeight(): void {
    const windowHeight = window.innerHeight;
    const navbarHeight = 64; // Approximate navbar height
    const tabBarHeight = 48; // Tab bar height
    const editorToolbarHeight = 100; // Catalog selector + buttons + toolbar
    const bottomMargin = 16; // Small margin at bottom
    
    // Calculate available height
    let availableHeight = windowHeight - navbarHeight - tabBarHeight - editorToolbarHeight - bottomMargin;
    
    // If there are results, reserve space for them
    if (this.queryResult && !this.sqlEditorCollapsed) {
      availableHeight = Math.max(300, availableHeight * 0.4); // Editor takes 40% when results shown
    } else if (this.queryResult && this.sqlEditorCollapsed) {
      availableHeight = 60; // Collapsed bar height
    }
    
    this.editorHeight = Math.max(200, Math.min(600, availableHeight));
    
    // Update editor DOM height without re-creating instance
    if (this.editorView) {
      this.editorView.dom.style.height = `${this.editorHeight}px`;
    }
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

  private loadCatalogs(): void {
    if (!this.clusterId) {
      return;
    }

    this.loadingCatalogs = true;
    this.nodeService.getCatalogs().subscribe({
      next: (catalogs) => {
        this.catalogs = catalogs;
        this.loadingCatalogs = false;
        // Auto-select first catalog if available (always select if only one or first available)
        if (catalogs.length > 0) {
          // If no catalog selected or selected catalog not in list, select first one
          if (!this.selectedCatalog || !catalogs.includes(this.selectedCatalog)) {
            this.selectedCatalog = catalogs[0];
            // Load databases for selected catalog
            this.loadDatabases();
          } else if (this.selectedCatalog && catalogs.includes(this.selectedCatalog)) {
            // If catalog is already selected and still in list, just refresh databases
            this.loadDatabases();
          }
        } else {
          this.selectedCatalog = '';
          this.databases = [];
        }
      },
      error: (error) => {
        this.loadingCatalogs = false;
        console.error('Failed to load catalogs:', error);
      },
    });
  }

  onCatalogChange(catalog?: string): void {
    // When catalog changes, reload databases for that catalog
    const newCatalog = catalog !== undefined ? catalog : this.selectedCatalog;
    
    // Clear database selection and list
    this.selectedDatabase = null;
    this.databases = [];
    
    if (newCatalog) {
      // Small delay to ensure catalog selection is updated
      setTimeout(() => {
        this.loadDatabases();
      }, 100);
    }
  }

  private loadDatabases(): void {
    if (!this.clusterId || !this.selectedCatalog) {
      this.databases = [];
      return;
    }

    this.loadingDatabases = true;
    this.nodeService.getDatabases(this.selectedCatalog).subscribe({
      next: (databases) => {
        this.databases = databases || [];
        this.loadingDatabases = false;
        
      },
      error: (error) => {
        this.loadingDatabases = false;
        this.databases = [];
        console.error('Failed to load databases:', error);
        // Show error toast only if it's a real error (not just empty result)
        if (error.status !== 200 && error.status !== 404) {
          // Could optionally show a toast here, but maybe not needed for empty result
        }
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
            this.selectedCatalog = '';
            this.selectedDatabase = null;
            this.databases = [];
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
      // Check if there are available databases to select
      if (this.databases.length > 0) {
        this.toastrService.warning('请选择数据库', '提示');
        return;
      }
      // If no databases available, allow execution with empty database (不使用数据库)
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

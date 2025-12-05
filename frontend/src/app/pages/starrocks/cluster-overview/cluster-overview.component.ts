import { Component, OnInit, OnDestroy, AfterViewInit, ChangeDetectionStrategy, ChangeDetectorRef } from '@angular/core';
import { Router } from '@angular/router';
import { Subject, interval } from 'rxjs';
import { takeUntil, switchMap, skip } from 'rxjs/operators';
import { NbToastrService, NbThemeService } from '@nebular/theme';
import { CountUp } from 'countup.js';
import {
  OverviewService,
  ExtendedClusterOverview,
  HealthCard,
  PerformanceTrends,
  ResourceTrends,
  DataStatistics,
  CapacityPrediction,
  TopTableBySize,
  TopTableByAccess,
  CompactionDetailStats,
} from '../../../@core/data/overview.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';
import { AuthService } from '../../../@core/data/auth.service';

@Component({
  selector: 'ngx-cluster-overview',
  templateUrl: './cluster-overview.component.html',
  styleUrls: ['./cluster-overview.component.scss'],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class ClusterOverviewComponent implements OnInit, OnDestroy, AfterViewInit {
  overview: ExtendedClusterOverview | null = null;
  healthCards: HealthCard[] = [];
  performanceTrends: PerformanceTrends | null = null;
  resourceTrends: ResourceTrends | null = null;
  dataStatistics: DataStatistics | null = null;
  capacityPrediction: CapacityPrediction | null = null;
  compactionDetails: CompactionDetailStats | null = null;
  
  activeSessions: number = 0;
  runningQueries: number = 0;
  activeUsers1h: number = 0;
  activeUsers24h: number = 0;
  
  timeRange: string = '1h';
  loading = false;
  autoRefresh = false; // Default: disabled (will be enabled when interval is selected)
  refreshInterval: number | 'off' = 'off'; // Default: off (Grafana style)

  // Latency percentile selection
  selectedLatencyPercentile: 'P50' | 'P95' | 'P99' = 'P99';
  latencyPercentileOptions = [
    { label: 'P50 (中位数)', value: 'P50' },
    { label: 'P95', value: 'P95' },
    { label: 'P99 (长尾)', value: 'P99' }
  ];
  
  // Disk/Cache metric selection
  selectedDiskMetric: 'percentage' | 'bytes' = 'percentage';
  
  // Expose Math to template
  Math = Math;
  
  // Nebular theme colors (dynamically loaded from theme)
  chartColors: any = {};
  
  private destroy$ = new Subject<void>();

  // Time range options
  timeRangeOptions = [
    { label: '1 Hour', value: '1h' },
    { label: '6 Hours', value: '6h' },
    { label: '24 Hours', value: '24h' },
    { label: '3 Days', value: '3d' },
  ];

  // Refresh interval options (Grafana style)
  refreshIntervalOptions = [
    { label: '关闭', value: 'off' },
    { label: '15秒', value: 15 },
    { label: '30秒', value: 30 },
    { label: '1分钟', value: 60 },
  ];

  /**
   * ✅ AUTO-REFRESH IMPLEMENTATION:
   * - When user selects a refresh interval (15s, 30s, 1m), the component calls setupAutoRefresh()
   * - setupAutoRefresh() creates an interval Observable that fires at the selected interval
   * - Each interval trigger calls loadOverview(false) for silent refresh (no loading spinner)
   * - User can toggle auto-refresh on/off or change interval anytime
   * - When cluster changes, auto-refresh is restarted
   * - Refresh intervals can be configured in the refreshIntervalOptions array above
   */
  
  constructor(
    private overviewService: OverviewService,
    private clusterContext: ClusterContextService,
    private router: Router,
    private toastr: NbToastrService,
    private themeService: NbThemeService,
    private authService: AuthService,
    private cdr: ChangeDetectorRef,
  ) {}

  ngOnInit() {
    // Load Nebular theme colors
    this.themeService.getJsTheme()
      .pipe(takeUntil(this.destroy$))
      .subscribe(theme => {
        const colors = theme.variables;
        this.chartColors = {
          primary: colors.primary,
          success: colors.success,
          info: colors.info,
          warning: colors.warning,
          danger: colors.danger,
          cardBg: colors.cardBackgroundColor,
          textBasic: colors.textBasicColor,
          textHint: colors.textHintColor,
          border: colors.borderColor || colors.dividerColor,
        };
        this.cdr.markForCheck();
      });

    // Initialize overview data loading
    this.loadOverview();
    this.setupAutoRefresh();

    // Listen to active cluster changes
    this.clusterContext.activeCluster$
      .pipe(
        skip(1), // Skip initial value
        takeUntil(this.destroy$)
      )
      .subscribe(cluster => {
        // Active cluster changed, reload overview
        this.loadOverview();
        this.setupAutoRefresh();
        this.cdr.markForCheck();
      });
  }

  ngAfterViewInit() {
    // Animate numbers after view is initialized
    setTimeout(() => this.animateNumbers(), 100);
  }

  ngOnDestroy() {
    this.destroy$.next();
    this.destroy$.complete();
  }

  setupAutoRefresh() {
    // Clear any existing subscription
    this.destroy$.next();
    
    // Only setup if interval is a number (not 'off')
    if (typeof this.refreshInterval === 'number' && this.refreshInterval > 0) {
      interval(this.refreshInterval * 1000)
        .pipe(
          takeUntil(this.destroy$),
        )
        .subscribe(() => {
          // Stop auto-refresh if user is not authenticated (logged out)
          if (!this.authService.isAuthenticated()) {
            this.autoRefresh = false;
            this.refreshInterval = 'off';
            this.destroy$.next();
            return;
          }
          if (this.autoRefresh) {
            this.loadOverview(false); // silent refresh
          }
        });
    }
  }

  loadOverview(showLoading: boolean = true) {
    if (showLoading) {
      this.loading = true;
      this.cdr.markForCheck();
    }

    // Load data from real API - no clusterId needed, backend will get from active cluster
    this.overviewService.getExtendedClusterOverview(this.timeRange)
      .subscribe({
        next: (overview) => {
          this.overview = overview;
          // Transform data using service methods
          this.healthCards = this.overviewService.transformToHealthCards(overview);
          this.performanceTrends = overview.performance_trends;
          this.resourceTrends = overview.resource_trends;
          this.dataStatistics = this.overviewService.transformDataStatistics(overview);
          this.capacityPrediction = overview.capacity || null;
          
          // Extract session stats
          if (overview.sessions) {
            this.activeSessions = overview.sessions.current_connections || 0;
            this.runningQueries = overview.sessions.running_queries?.length || 0;
            this.activeUsers1h = overview.sessions.active_users_1h || 0;
            this.activeUsers24h = overview.sessions.active_users_24h || 0;
          }
          
          this.loading = false;
          
          // Animate numbers after data is loaded
          setTimeout(() => this.animateNumbers(), 100);
          this.cdr.markForCheck();
        },
        error: (err) => {
          console.error('[ClusterOverview] Failed to load cluster overview:', err);
          console.error('[ClusterOverview] Error details:', {
            status: err.status,
            statusText: err.statusText,
            message: err.message,
            error: err.error
          });
          
          let errorMsg = '加载集群概览失败';
          
          // Show backend error message directly
          if (err.error?.message) {
            errorMsg = err.error.message;
          } else if (err.status === 0) {
            errorMsg = '无法连接到服务器';
          } else if (err.status === 404) {
            errorMsg = '未找到数据或未激活集群';
          } else if (err.status === 401) {
            errorMsg = '没有权限执行此操作';
          }
          
          this.toastr.danger(errorMsg, '错误');
          this.loading = false;
          this.cdr.markForCheck();
        }
      });
    
    // Load compaction detail stats separately
    this.overviewService.getCompactionDetailStats(this.timeRange)
      .subscribe({
        next: (details) => {
          this.compactionDetails = details;
          this.cdr.markForCheck();
        },
        error: (err) => {
          console.warn('[ClusterOverview] Failed to load compaction details:', err);
          // Don't show error to user - compaction details are optional
          this.compactionDetails = null;
          this.cdr.markForCheck();
        }
      });
  }

  onTimeRangeChange(range: string) {
    this.timeRange = range;
    this.loadOverview();
  }

  // Grafana-style: selecting an interval automatically enables auto-refresh
  // Selecting 'off' disables auto-refresh
  onRefreshIntervalChange(interval: number | 'off') {
    this.refreshInterval = interval;
    
    if (interval === 'off') {
      // Disable auto-refresh
      this.autoRefresh = false;
      this.setupAutoRefresh(); // This will clear the interval
    } else {
      // Enable auto-refresh with selected interval
      this.autoRefresh = true;
      this.setupAutoRefresh();
    }
  }

  onManualRefresh() {
    this.loadOverview();
  }

  // Navigation methods
  handleCardClick(card: HealthCard) {
    if (card.cardId === 'latency_percentile') {
      this.cycleLatencyPercentile();
    } else if (card.cardId === 'disk_cache_metric') {
      this.cycleDiskMetric();
    } else if (card.cardId === 'load_jobs') {
      this.navigateToLoadJobs();
    } else if (card.cardId === 'sessions') {
      this.navigateToSessions();
    } else if (card.cardId === 'compactions' || card.cardId === 'compaction_score') {
      this.navigateToCompactions();
    } else {
      this.navigateToCard(card);
    }
  }

  navigateToCard(card: HealthCard) {
    if (card.navigateTo) {
      this.router.navigate([card.navigateTo]);
    }
  }

  navigateToQueries() {
    this.router.navigate(['/pages/starrocks/queries/execution']);
  }

  navigateToBackends() {
    this.router.navigate(['/pages/starrocks/backends']);
  }

  navigateToMaterializedViews() {
    this.router.navigate(['/pages/starrocks/materialized-views']);
  }

  // Computed properties for template optimization (avoid repeated array operations)
  get topTablesBySize(): any[] {
    return (this.dataStatistics?.topTablesBySize || []).slice(0, 10);
  }

  get topTablesByAccess(): any[] {
    return (this.dataStatistics?.topTablesByAccess || []).slice(0, 10);
  }

  // Helper methods
  
  /**
   * Format bytes to human-readable size string (with unit)
   * Legacy method for backward compatibility
   */
  formatBytes(bytes: number): string {
    const formatted = this.formatBytesAdaptive(bytes);
    return `${formatted.value} ${formatted.unit}`;
  }
  
  /**
   * Format bytes to human-readable size with adaptive unit
   * 自适应单位显示：大于1024T显示P，大于1024G显示T，以此类推
   */
  formatBytesAdaptive(bytes: number): { value: string; unit: string } {
    if (bytes === 0) return { value: '0', unit: 'B' };
    
    const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    const k = 1024;
    
    let unitIndex = 0;
    let value = bytes;
    
    while (value >= k && unitIndex < units.length - 1) {
      value /= k;
      unitIndex++;
    }
    
    // Format value: show 1 decimal place for values < 10, otherwise round
    const formattedValue = value < 10 ? value.toFixed(1) : Math.round(value).toString();
    
    return {
      value: formattedValue,
      unit: units[unitIndex]
    };
  }
  
  /**
   * Format milliseconds to human-readable duration with adaptive unit
   * 自适应时间单位：大于60s显示分钟，大于60分显示小时，以此类推
   */
  formatDuration(ms: number): { value: string; unit: string } {
    if (ms === 0) return { value: '0', unit: 'ms' };
    if (ms < 0) return { value: '0', unit: 'ms' };
    
    const units = [
      { name: 'ms', threshold: 1000 },
      { name: 's', threshold: 60 },
      { name: '分', threshold: 60 },
      { name: '时', threshold: 24 },
      { name: '天', threshold: 7 },
      { name: '周', threshold: Number.MAX_VALUE }
    ];
    
    let value = ms;
    let unitIndex = 0;
    
    // Convert ms to seconds first
    if (value >= 1000) {
      value /= 1000;
      unitIndex = 1;
    }
    
    // Then convert through other units
    while (unitIndex < units.length - 1 && value >= units[unitIndex].threshold) {
      value /= units[unitIndex].threshold;
      unitIndex++;
    }
    
    // Format value: show 1 decimal place for values < 10, otherwise round
    const formattedValue = value < 10 ? value.toFixed(1) : Math.round(value).toString();
    
    return {
      value: formattedValue,
      unit: units[unitIndex].name
    };
  }

  /**
   * Format number with K/M suffix (legacy method for backward compatibility)
   */
  formatNumber(num: number): string {
    if (num >= 1000000) {
      return (num / 1000000).toFixed(2) + 'M';
    } else if (num >= 1000) {
      return (num / 1000).toFixed(2) + 'K';
    }
    return num.toString();
  }

  /**
   * Format duration as string (legacy method for backward compatibility)
   * Use formatDuration() for adaptive unit formatting
   */
  formatDurationString(ms: number): string {
    const formatted = this.formatDuration(ms);
    return `${formatted.value}${formatted.unit}`;
  }

  getStatusIcon(status: string): string {
    switch (status) {
      case 'success': return 'checkmark-circle-2-outline';
      case 'warning': return 'alert-triangle-outline';
      case 'danger': return 'close-circle-outline';
      case 'info': return 'info-outline';
      default: return 'info-outline';
    }
  }

  getTrendIcon(trend: number): string {
    if (trend > 0) return 'trending-up-outline';
    if (trend < 0) return 'trending-down-outline';
    return 'minus-outline';
  }

  getTrendColor(trend: number): string {
    if (trend > 0) return 'success';
    if (trend < 0) return 'danger';
    return 'basic';
  }

  // Get latency value based on selected percentile
  getSelectedLatencyValue(): number {
    if (!this.performanceTrends) return 0;
    
    const trends = this.performanceTrends;
    switch (this.selectedLatencyPercentile) {
      case 'P50':
        return trends.latency_p50?.length > 0 
          ? trends.latency_p50[trends.latency_p50.length - 1].value 
          : 0;
      case 'P95':
        return trends.latency_p95?.length > 0 
          ? trends.latency_p95[trends.latency_p95.length - 1].value 
          : 0;
      case 'P99':
        return trends.latency_p99?.length > 0 
          ? trends.latency_p99[trends.latency_p99.length - 1].value 
          : 0;
      default:
        return 0;
    }
  }

  getSelectedLatencyStatus(): string {
    const latency = this.getSelectedLatencyValue();
    if (latency < 1000) return 'success';
    if (latency < 5000) return 'warning';
    return 'danger';
  }

  onLatencyPercentileChange(percentile: 'P50' | 'P95' | 'P99'): void {
    this.selectedLatencyPercentile = percentile;
  }

  // Cycle through latency percentiles on click
  cycleLatencyPercentile(): void {
    const options: Array<'P50' | 'P95' | 'P99'> = ['P50', 'P95', 'P99'];
    const currentIndex = options.indexOf(this.selectedLatencyPercentile);
    const nextIndex = (currentIndex + 1) % options.length;
    this.selectedLatencyPercentile = options[nextIndex];
  }

  getLatencyCardTitle(): string {
    return `${this.selectedLatencyPercentile} 延迟`;
  }
  
  // Cycle through disk/cache metrics on click
  cycleDiskMetric(): void {
    this.selectedDiskMetric = this.selectedDiskMetric === 'percentage' ? 'bytes' : 'percentage';
  }
  
  getDiskCardValue(): string {
    if (!this.capacityPrediction) return '0';
    const capacity = this.capacityPrediction;
    
    if (this.selectedDiskMetric === 'percentage') {
      return Math.round(capacity.disk_usage_pct).toString();
    } else {
      // bytes - use adaptive unit formatting
      const formatted = this.formatBytesAdaptive(capacity.disk_used_bytes);
      return formatted.value;
    }
  }
  
  getDiskCardUnit(): string {
    if (this.selectedDiskMetric === 'percentage') {
      return '%';
    } else {
      const formatted = this.formatBytesAdaptive(this.capacityPrediction?.disk_used_bytes || 0);
      return formatted.unit;
    }
  }
  
  getDiskCardTitle(): string {
    return this.selectedDiskMetric === 'percentage' ? '本地磁盘' : '缓存使用';
  }
  
  getDiskCardDescription(): string {
    return this.selectedDiskMetric === 'percentage' 
      ? '计算节点本地磁盘最大使用率（点击切换）'
      : '使用率最高节点的本地缓存数据量（点击切换）';
  }
  
  getDiskCardStatus(): string {
    if (!this.capacityPrediction) return 'info';
    const capacity = this.capacityPrediction;
    
    if (this.selectedDiskMetric === 'percentage') {
      return capacity.disk_usage_pct > 80 ? 'warning' : 'info';
    } else {
      // For bytes, check if usage is high relative to real data
      const usageRatio = capacity.real_data_size_bytes > 0 
        ? capacity.disk_used_bytes / capacity.real_data_size_bytes 
        : 0;
      return usageRatio > 2 ? 'warning' : 'info'; // Warn if cache is 2x real data
    }
  }

  // ECharts Configuration Methods (使用 ngx-admin 兼容的渐变样式)

  /**
   * Generate unified series configuration for smooth line chart with area fill
   * 模仿StreamLake的轻量级风格：极细线条 + 极淡面积填充
   */
  private getLineSeries(name: string, data: any[], color: string, withArea: boolean = true): any {
    const baseSeries = {
      name,
      type: 'line',
      smooth: true,
      symbol: 'circle',
      symbolSize: 0,  // Hide symbols by default
      showSymbol: false,
      sampling: 'lttb',
      lineStyle: { 
        width: 1.5,  // Very thin line like StreamLake
        color,
      },
      emphasis: {
        focus: 'series',
        lineStyle: {
          width: 2,
        },
      },
      data,
    };

    if (withArea) {
      return {
        ...baseSeries,
        areaStyle: {
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: this.hexToRgba(color, 0.15) },  // Very subtle gradient
              { offset: 1, color: this.hexToRgba(color, 0.01) },  // Almost transparent
            ],
          },
        },
      };
    }

    return baseSeries;
  }

  private getBaseChartOptions(color: string): any {
    return {
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        top: '12%',
        containLabel: true,
      },
      tooltip: {
        trigger: 'axis',
        axisPointer: {
          type: 'line',
          lineStyle: {
            color: '#d0d7e3',
            width: 1,
            type: 'solid',
          },
        },
        backgroundColor: 'rgba(255, 255, 255, 0.96)',
        borderColor: '#e4e9f2',
        borderWidth: 1,
        textStyle: {
          color: '#2e3a59',
          fontSize: 11,
        },
        padding: [8, 12],
        extraCssText: 'box-shadow: 0 2px 8px rgba(0,0,0,0.08);',
      },
      xAxis: {
        type: 'category',
        boundaryGap: false,
        show: true,
        axisLabel: {
          show: true,
          color: '#8f9bb3',
          fontSize: 11,
          margin: 8,
        },
        axisLine: {
          show: true,
          lineStyle: {
            color: '#e4e9f2',
            width: 1,
          },
        },
        axisTick: {
          show: true,
          lineStyle: {
            color: '#e4e9f2',
          },
        },
      },
      yAxis: {
        type: 'value',
        show: true,
        axisLabel: {
          show: true,
          color: '#8f9bb3',
          fontSize: 11,
          margin: 8,
        },
        axisLine: {
          show: true,
          lineStyle: {
            color: '#e4e9f2',
            width: 1,
          },
        },
        axisTick: {
          show: true,
          lineStyle: {
            color: '#e4e9f2',
          },
        },
        splitLine: {
          show: true,
          lineStyle: {
            color: '#e4e9f2',
            width: 1,
            type: 'solid',
          },
        },
      },
    };
  }

  getQpsChartOptions(): any {
    if (!this.performanceTrends || !this.performanceTrends.qps || !this.performanceTrends.rps) {
      return {};
    }

    const qpsData = this.performanceTrends.qps;
    const rpsData = this.performanceTrends.rps;
    const times = qpsData.map(d => new Date(d.timestamp).toLocaleTimeString());
    const qpsValues = qpsData.map(d => d.value);
    const rpsValues = rpsData.map(d => d.value);
    const qpsColor = this.chartColors.primary || '#3366ff';
    const rpsColor = this.chartColors.success || '#00d68f';

    return {
      ...this.getBaseChartOptions(qpsColor),
      legend: {
        data: ['QPS (查询/秒)', 'RPS (请求/秒)'],
        textStyle: { color: this.chartColors.textBasic },
        top: 0,
      },
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        borderColor: qpsColor,
        textStyle: { color: this.chartColors.textBasic },
      },
      xAxis: {
        ...this.getBaseChartOptions(qpsColor).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(qpsColor).yAxis,
        name: '次数/秒',
        nameTextStyle: { color: this.chartColors.textHint },
      },
      series: [
        this.getLineSeries('QPS (查询/秒)', qpsValues, qpsColor),
        this.getLineSeries('RPS (请求/秒)', rpsValues, rpsColor),
      ],
    };
  }

  getLatencyChartOptions(): any {
    if (!this.performanceTrends || !this.performanceTrends.latency_p50 || 
        !this.performanceTrends.latency_p95 || !this.performanceTrends.latency_p99) {
      return {};
    }

    const p50Data = this.performanceTrends.latency_p50;
    const p95Data = this.performanceTrends.latency_p95;
    const p99Data = this.performanceTrends.latency_p99;
    const times = p50Data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const p50Values = p50Data.map(d => d.value);
    const p95Values = p95Data.map(d => d.value);
    const p99Values = p99Data.map(d => d.value);
    const p50Color = this.chartColors.success || '#00d68f';
    const p95Color = this.chartColors.warning || '#ffaa00';
    const p99Color = this.chartColors.danger || '#ff3d71';

    return {
      ...this.getBaseChartOptions(p99Color),
      legend: {
        data: ['P50 (中位数)', 'P95', 'P99 (最差)'],
        textStyle: { color: this.chartColors.textBasic },
        top: 0,
      },
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        borderColor: p99Color,
        textStyle: { color: this.chartColors.textBasic },
        formatter: (params: any) => {
          let result = params[0].axisValue + '<br/>';
          params.forEach((item: any) => {
            result += `${item.marker} ${item.seriesName}: ${item.value.toFixed(2)} ms<br/>`;
          });
          return result;
        },
      },
      xAxis: {
        ...this.getBaseChartOptions(p99Color).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(p99Color).yAxis,
        name: '延迟 (ms)',
        nameTextStyle: { color: this.chartColors.textHint },
        axisLabel: { 
          ...this.getBaseChartOptions(p99Color).yAxis.axisLabel,
          formatter: (value: number) => value.toFixed(0),
        },
      },
      series: [
        this.getLineSeries('P50 (中位数)', p50Values, p50Color, false),  // No area fill
        this.getLineSeries('P95', p95Values, p95Color, false),  // No area fill
        this.getLineSeries('P99 (最差)', p99Values, p99Color, true),  // With area fill
      ],
    };
  }

  getCpuChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.cpu_usage) {
      return {};
    }

    const data = this.resourceTrends.cpu_usage;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);
    const color = this.chartColors.success || '#00d68f';

    return {
      ...this.getBaseChartOptions(color),
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        borderColor: color,
        textStyle: { color: this.chartColors.textBasic },
      },
      xAxis: {
        ...this.getBaseChartOptions(color).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(color).yAxis,
        max: 100,
      },
      series: [
        this.getLineSeries('CPU Usage (%)', values, color, true),
      ],
    };
  }

  getMemoryChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.memory_usage) {
      return {};
    }

    const data = this.resourceTrends.memory_usage;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);
    const color = this.chartColors.info || '#0095ff';

    return {
      ...this.getBaseChartOptions(color),
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        borderColor: color,
        textStyle: { color: this.chartColors.textBasic },
      },
      xAxis: {
        ...this.getBaseChartOptions(color).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(color).yAxis,
        max: 100,
      },
      series: [
        this.getLineSeries('Memory Usage (%)', values, color, true),
      ],
    };
  }

  getDiskChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.disk_usage) {
      return {};
    }

    const data = this.resourceTrends.disk_usage;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);
    const color = this.chartColors.warning || '#ffaa00';

    return {
      ...this.getBaseChartOptions(color),
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        borderColor: color,
        textStyle: { color: this.chartColors.textBasic },
      },
      xAxis: {
        ...this.getBaseChartOptions(color).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(color).yAxis,
        max: 100,
      },
      series: [
        this.getLineSeries('Disk Usage (%)', values, color, true),
      ],
    };
  }

  // JVM Heap Memory Usage Chart (FE JVM堆内存使用率)
  getJvmHeapChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.jvm_heap_usage) {
      return {};
    }
    
    const data = this.resourceTrends.jvm_heap_usage;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);
    const color = this.chartColors.info || '#0095ff';

    return {
      ...this.getBaseChartOptions(color),
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        borderColor: color,
        textStyle: { color: this.chartColors.textBasic },
        formatter: (params: any) => {
          const value = params[0].value;
          return `${params[0].axisValue}<br/>${params[0].marker} JVM 堆内存: ${value.toFixed(1)}%`;
        },
      },
      xAxis: {
        ...this.getBaseChartOptions(color).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(color).yAxis,
        max: 100,
        name: '使用率 (%)',
        nameTextStyle: { color: this.chartColors.textHint },
        axisLabel: { 
          ...this.getBaseChartOptions(color).yAxis.axisLabel,
          formatter: (value: number) => `${value}%`,
        },
      },
      series: [
        {
          name: 'JVM Heap Usage (%)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: color,
            borderWidth: 2,
            borderColor: '#fff',
          },
          lineStyle: { width: 3 },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(color, 0.5) },
                { offset: 1, color: this.hexToRgba(color, 0.05) },
              ],
            },
          },
          data: values,
          markLine: {
            silent: true,
            lineStyle: { color: this.chartColors.danger, type: 'dashed', width: 2 },
            data: [{ yAxis: 80, label: { formatter: '警戒线 80%', color: this.chartColors.danger } }],
          },
        },
      ],
    };
  }

  // Combined CPU/Memory/Disk Chart (三合一资源图表)
  getResourceChartOptions(): any {
    if (!this.resourceTrends || 
        !this.resourceTrends.cpu_usage || 
        !this.resourceTrends.memory_usage || 
        !this.resourceTrends.disk_usage) {
      return {};
    }

    const cpuData = this.resourceTrends.cpu_usage;
    const memoryData = this.resourceTrends.memory_usage;
    const diskData = this.resourceTrends.disk_usage;
    
    const times = cpuData.map(d => new Date(d.timestamp).toLocaleTimeString());
    const cpuValues = cpuData.map(d => d.value);
    const memoryValues = memoryData.map(d => d.value);
    const diskValues = diskData.map(d => d.value);
    
    const cpuColor = this.chartColors.primary || '#3366ff';
    const memoryColor = this.chartColors.danger || '#ff3d71';
    const diskColor = this.chartColors.success || '#00d68f';

    return {
      ...this.getBaseChartOptions(cpuColor),
      xAxis: {
        ...this.getBaseChartOptions(cpuColor).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(cpuColor).yAxis,
        max: 100,
        axisLabel: {
          ...this.getBaseChartOptions(cpuColor).yAxis.axisLabel,
          formatter: '{value}%',
        },
      },
      legend: {
        data: ['CPU', 'Memory', 'Disk'],
        top: 0,
        textStyle: { color: this.chartColors.textBasic },
      },
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        textStyle: { color: this.chartColors.textBasic },
        axisPointer: {
          type: 'cross',
        },
        formatter: (params: any) => {
          let result = params[0].axisValue + '<br/>';
          params.forEach((param: any) => {
            result += `${param.marker} ${param.seriesName}: ${param.value.toFixed(1)}%<br/>`;
          });
          return result;
        },
      },
      series: [
        {
          name: 'CPU',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: cpuColor,
          },
          lineStyle: {
            width: 2,
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(cpuColor, 0.2) },
                { offset: 1, color: this.hexToRgba(cpuColor, 0.02) },
              ],
            },
          },
          data: cpuValues,
        },
        {
          name: 'Memory',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: memoryColor,
          },
          lineStyle: {
            width: 2,
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(memoryColor, 0.2) },
                { offset: 1, color: this.hexToRgba(memoryColor, 0.02) },
              ],
            },
          },
          data: memoryValues,
        },
        {
          name: 'Disk',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: diskColor,
          },
          lineStyle: {
            width: 2,
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(diskColor, 0.2) },
                { offset: 1, color: this.hexToRgba(diskColor, 0.02) },
              ],
            },
          },
          data: diskValues,
        },
      ],
    };
  }

  getNetworkChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.network_tx || !this.resourceTrends.network_rx) {
      return {};
    }

    const txData = this.resourceTrends.network_tx;
    const rxData = this.resourceTrends.network_rx;
    const times = txData.map(d => new Date(d.timestamp).toLocaleTimeString());
    const txValues = txData.map(d => d.value / 1024 / 1024); // Convert to MB/s
    const rxValues = rxData.map(d => d.value / 1024 / 1024);
    
    const txColor = this.chartColors.primary || '#3366ff';
    const rxColor = this.chartColors.success || '#00d68f';

    return {
      ...this.getBaseChartOptions(txColor),
      xAxis: {
        ...this.getBaseChartOptions(txColor).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(txColor).yAxis,
      },
      legend: {
        data: ['TX (Send)', 'RX (Receive)'],
        textStyle: { color: this.chartColors.textBasic },
      },
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        textStyle: { color: this.chartColors.textBasic },
      },
      series: [
        {
          name: 'TX (Send)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: txColor,
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(txColor, 0.3) },
                { offset: 1, color: this.hexToRgba(txColor, 0.05) },
              ],
            },
          },
          data: txValues,
        },
        {
          name: 'RX (Receive)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: rxColor,
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(rxColor, 0.3) },
                { offset: 1, color: this.hexToRgba(rxColor, 0.05) },
              ],
            },
          },
          data: rxValues,
        },
      ],
    };
  }

  getIoChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.io_read || !this.resourceTrends.io_write) {
      return {};
    }

    const readData = this.resourceTrends.io_read;
    const writeData = this.resourceTrends.io_write;
    const times = readData.map(d => new Date(d.timestamp).toLocaleTimeString());
    const readValues = readData.map(d => d.value / 1024 / 1024); // Convert to MB/s
    const writeValues = writeData.map(d => d.value / 1024 / 1024);
    
    const readColor = this.chartColors.success || '#00d68f';
    const writeColor = this.chartColors.warning || '#ffaa00';

    return {
      ...this.getBaseChartOptions(readColor),
      xAxis: {
        ...this.getBaseChartOptions(readColor).xAxis,
        data: times,
      },
      yAxis: {
        ...this.getBaseChartOptions(readColor).yAxis,
      },
      legend: {
        data: ['Read', 'Write'],
        textStyle: { color: this.chartColors.textBasic },
      },
      tooltip: {
        trigger: 'axis',
        backgroundColor: this.chartColors.cardBg,
        textStyle: { color: this.chartColors.textBasic },
      },
      series: [
        {
          name: 'Read',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: readColor,
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(readColor, 0.3) },
                { offset: 1, color: this.hexToRgba(readColor, 0.05) },
              ],
            },
          },
          data: readValues,
        },
        {
          name: 'Write',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: writeColor,
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: this.hexToRgba(writeColor, 0.3) },
                { offset: 1, color: this.hexToRgba(writeColor, 0.05) },
              ],
            },
          },
          data: writeValues,
        },
      ],
    };
  }

  getTrendLabel(trend: string): string {
    const labels: { [key: string]: string } = {
      'increasing': '增长中',
      'decreasing': '下降中',
      'stable': '稳定'
    };
    return labels[trend] || trend;
  }

  /**
   * Animate numbers using CountUp.js for visual appeal
   */
  private animateNumbers() {
    // Animate health card numbers
    this.healthCards.forEach((card, index) => {
      const element = document.getElementById(`card-value-${index}`);
      if (element && card.value) {
        // Extract numeric value from card.value (may include text like "8/8")
        const numericValue = this.extractNumericValue(card.value);
        if (numericValue !== null) {
          const countUp = new CountUp(element, numericValue, {
            duration: 2,
            useEasing: true,
            separator: ',',
            decimal: '.',
            decimalPlaces: this.getDecimalPlaces(numericValue),
          });
          if (!countUp.error) {
            countUp.start();
          }
        }
      }
    });

    // Animate data statistics numbers
    if (this.dataStatistics) {
      this.animateDataStatElement('stat-databases', this.dataStatistics.databaseCount);
      this.animateDataStatElement('stat-tables', this.dataStatistics.tableCount);
      this.animateDataStatElement('stat-data-size', this.dataStatistics.totalDataSizeBytes / (1024 * 1024 * 1024 * 1024)); // TB
    }
  }

  private animateDataStatElement(elementId: string, value: number) {
    const element = document.getElementById(elementId);
    if (element) {
      const countUp = new CountUp(element, value, {
        duration: 2,
        useEasing: true,
        separator: ',',
        decimal: '.',
        decimalPlaces: this.getDecimalPlaces(value),
      });
      if (!countUp.error) {
        countUp.start();
      }
    }
  }

  private extractNumericValue(value: string | number): number | null {
    // Handle cases like "8/8", "75%", "1234", "2.5"
    if (typeof value === 'number') {
      return value;
    }
    
    const match = value.toString().match(/^(\d+\.?\d*)/);
    return match ? parseFloat(match[1]) : null;
  }

  private getDecimalPlaces(value: number): number {
    if (value >= 100) return 0;
    if (value >= 10) return 1;
    return 2;
  }

  /**
   * Convert hex color to RGBA
   * Used for chart gradient effects with Nebular theme colors
   */
  private hexToRgba(hex: string, alpha: number): string {
    if (!hex) return `rgba(51, 102, 255, ${alpha})`; // fallback color
    
    // Remove # if present
    hex = hex.replace('#', '');
    
    // Parse hex values
    const r = parseInt(hex.substring(0, 2), 16);
    const g = parseInt(hex.substring(2, 4), 16);
    const b = parseInt(hex.substring(4, 6), 16);
    
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }

  /**
   * Get CSS class for compaction score status indication
   * 
   * @param score Compaction score value
   * @returns CSS class name for styling
   */
  getScoreStatusClass(score: number): string {
    if (score > 100) return 'text-danger';   // Critical
    if (score > 50) return 'text-warning';   // Warning
    return 'text-success';                   // Normal
  }

  // 导航到compaction列表页面
  navigateToCompactions() {
    this.router.navigate(['/pages/starrocks/system'], { 
      queryParams: { 
        function: 'compactions',
        from: 'overview'  // 标记来源，用于返回功能
      } 
    });
  }

  navigateToLoadJobs(event?: Event) {
    if (event) {
      event.stopPropagation();
    }
    this.router.navigate(['/pages/starrocks/system'], { 
      queryParams: { 
        function: 'loads',
        from: 'overview'  // 标记来源，用于返回功能
      } 
    });
  }

  navigateToSessions() {
    this.router.navigate(['/pages/starrocks/sessions']);
  }
}

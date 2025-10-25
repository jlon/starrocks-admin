import { Component, OnInit, OnDestroy } from '@angular/core';
import { Router } from '@angular/router';
import { Subject, interval } from 'rxjs';
import { takeUntil, switchMap } from 'rxjs/operators';
import {
  OverviewService,
  ClusterOverview,
  HealthCard,
  PerformanceTrends,
  ResourceTrends,
  DataStatistics,
  CapacityPrediction,
  TopTableBySize,
  TopTableByAccess,
  SlowQuery,
} from '../../../@core/data/overview.service';
import { ClusterContextService } from '../../../@core/data/cluster-context.service';

@Component({
  selector: 'ngx-cluster-overview',
  templateUrl: './cluster-overview.component.html',
  styleUrls: ['./cluster-overview.component.scss'],
})
export class ClusterOverviewComponent implements OnInit, OnDestroy {
  overview: ClusterOverview | null = null;
  healthCards: HealthCard[] = [];
  performanceTrends: PerformanceTrends | null = null;
  resourceTrends: ResourceTrends | null = null;
  dataStatistics: DataStatistics | null = null;
  capacityPrediction: CapacityPrediction | null = null;
  
  clusterId: number = 0;
  timeRange: string = '1h';
  loading = false;
  autoRefresh = true;
  refreshInterval = 30; // seconds
  
  private destroy$ = new Subject<void>();

  // Time range options
  timeRangeOptions = [
    { label: '1 Hour', value: '1h' },
    { label: '6 Hours', value: '6h' },
    { label: '24 Hours', value: '24h' },
    { label: '3 Days', value: '3d' },
  ];

  // Refresh interval options
  refreshIntervalOptions = [
    { label: '15s', value: 15 },
    { label: '30s', value: 30 },
    { label: '1m', value: 60 },
    { label: 'Manual', value: 0 },
  ];

  constructor(
    private overviewService: OverviewService,
    private clusterContext: ClusterContextService,
    private router: Router,
  ) {}

  ngOnInit() {
    // For testing, load mock data immediately
    this.clusterId = 1;
    this.loadOverview();
    this.setupAutoRefresh();

    // Listen to active cluster changes (disabled for testing)
    /*
    this.clusterContext.activeCluster$
      .pipe(takeUntil(this.destroy$))
      .subscribe(cluster => {
        if (cluster) {
          this.clusterId = cluster.id;
          this.loadOverview();
          this.setupAutoRefresh();
        }
      });
    */
  }

  ngOnDestroy() {
    this.destroy$.next();
    this.destroy$.complete();
  }

  setupAutoRefresh() {
    if (this.refreshInterval > 0) {
      interval(this.refreshInterval * 1000)
        .pipe(
          takeUntil(this.destroy$),
        )
        .subscribe(() => {
          if (this.autoRefresh) {
            this.loadOverview(false); // silent refresh
          }
        });
    }
  }

  loadOverview(showLoading: boolean = true) {
    if (showLoading) {
      this.loading = true;
    }

    // Use mock data for testing
    this.loadMockData();
    this.loading = false;
    return;

    // Load all data in parallel (original code, disabled for mock)
    Promise.all([
      this.overviewService.getHealthCards(this.clusterId).toPromise(),
      this.overviewService.getPerformanceTrends(this.clusterId, this.timeRange).toPromise(),
      this.overviewService.getResourceTrends(this.clusterId, this.timeRange).toPromise(),
      this.overviewService.getDataStatistics(this.clusterId).toPromise(),
      this.overviewService.getCapacityPrediction(this.clusterId).toPromise(),
    ])
      .then(([healthCards, performanceTrends, resourceTrends, dataStatistics, capacityPrediction]) => {
        this.healthCards = healthCards || [];
        this.performanceTrends = performanceTrends;
        this.resourceTrends = resourceTrends;
        this.dataStatistics = dataStatistics;
        this.capacityPrediction = capacityPrediction;
        this.loading = false;
      })
      .catch(err => {
        console.error('Failed to load cluster overview:', err);
        this.loading = false;
      });
  }

  onTimeRangeChange(range: string) {
    this.timeRange = range;
    this.loadOverview();
  }

  onRefreshIntervalChange(interval: number) {
    this.refreshInterval = interval;
    this.autoRefresh = interval > 0;
    // Restart auto-refresh
    this.destroy$.next();
    this.setupAutoRefresh();
  }

  onManualRefresh() {
    this.loadOverview();
  }

  onToggleAutoRefresh() {
    this.autoRefresh = !this.autoRefresh;
  }

  // Navigation methods
  navigateToCard(card: HealthCard) {
    if (card.navigateTo) {
      this.router.navigate([card.navigateTo]);
    }
  }

  navigateToQueries() {
    this.router.navigate(['/pages/starrocks/queries']);
  }

  navigateToBackends() {
    this.router.navigate(['/pages/starrocks/backends']);
  }

  navigateToMaterializedViews() {
    this.router.navigate(['/pages/starrocks/materialized-views']);
  }

  // Helper methods
  formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  formatNumber(num: number): string {
    if (num >= 1000000) {
      return (num / 1000000).toFixed(2) + 'M';
    } else if (num >= 1000) {
      return (num / 1000).toFixed(2) + 'K';
    }
    return num.toString();
  }

  formatDuration(ms: number): string {
    if (ms < 1000) return ms + 'ms';
    if (ms < 60000) return (ms / 1000).toFixed(2) + 's';
    if (ms < 3600000) return (ms / 60000).toFixed(2) + 'm';
    return (ms / 3600000).toFixed(2) + 'h';
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

  // ECharts Configuration Methods (使用 ngx-admin 兼容的渐变样式)

  private getBaseChartOptions(color: string): any {
    return {
      grid: {
        left: '3%',
        right: '4%',
        bottom: '3%',
        top: '10%',
        containLabel: true,
      },
      tooltip: {
        trigger: 'axis',
        axisPointer: {
          type: 'cross',
        },
      },
      xAxis: {
        type: 'category',
        boundaryGap: false,
      },
      yAxis: {
        type: 'value',
      },
    };
  }

  getQpsChartOptions(): any {
    if (!this.performanceTrends || !this.performanceTrends.qpsSeries) {
      return {};
    }

    const data = this.performanceTrends.qpsSeries;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);

    return {
      ...this.getBaseChartOptions('#36f'),
      xAxis: {
        ...this.getBaseChartOptions('#36f').xAxis,
        data: times,
      },
      series: [
        {
          name: 'QPS',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: '#36f',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(51, 102, 255, 0.3)' },
                { offset: 1, color: 'rgba(51, 102, 255, 0.05)' },
              ],
            },
          },
          data: values,
        },
      ],
    };
  }

  getLatencyChartOptions(): any {
    if (!this.performanceTrends || !this.performanceTrends.latencyP99Series) {
      return {};
    }

    const data = this.performanceTrends.latencyP99Series;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);

    return {
      ...this.getBaseChartOptions('#f90'),
      xAxis: {
        ...this.getBaseChartOptions('#f90').xAxis,
        data: times,
      },
      series: [
        {
          name: 'P99 Latency (ms)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: '#f90',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(255, 153, 0, 0.3)' },
                { offset: 1, color: 'rgba(255, 153, 0, 0.05)' },
              ],
            },
          },
          data: values,
        },
      ],
    };
  }

  getErrorRateChartOptions(): any {
    if (!this.performanceTrends || !this.performanceTrends.errorRateSeries) {
      return {};
    }

    const data = this.performanceTrends.errorRateSeries;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);

    return {
      ...this.getBaseChartOptions('#f33'),
      xAxis: {
        ...this.getBaseChartOptions('#f33').xAxis,
        data: times,
      },
      series: [
        {
          name: 'Error Rate (%)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: '#f33',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(255, 51, 51, 0.3)' },
                { offset: 1, color: 'rgba(255, 51, 51, 0.05)' },
              ],
            },
          },
          data: values,
        },
      ],
    };
  }

  getCpuChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.cpuUsageSeries) {
      return {};
    }

    const data = this.resourceTrends.cpuUsageSeries;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);

    return {
      ...this.getBaseChartOptions('#3c6'),
      xAxis: {
        ...this.getBaseChartOptions('#3c6').xAxis,
        data: times,
      },
      yAxis: {
        type: 'value',
        max: 100,
      },
      series: [
        {
          name: 'CPU Usage (%)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: '#3c6',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(51, 204, 102, 0.3)' },
                { offset: 1, color: 'rgba(51, 204, 102, 0.05)' },
              ],
            },
          },
          data: values,
        },
      ],
    };
  }

  getMemoryChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.memoryUsageSeries) {
      return {};
    }

    const data = this.resourceTrends.memoryUsageSeries;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);

    return {
      ...this.getBaseChartOptions('#c6f'),
      xAxis: {
        ...this.getBaseChartOptions('#c6f').xAxis,
        data: times,
      },
      yAxis: {
        type: 'value',
        max: 100,
      },
      series: [
        {
          name: 'Memory Usage (%)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: '#c6f',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(204, 102, 255, 0.3)' },
                { offset: 1, color: 'rgba(204, 102, 255, 0.05)' },
              ],
            },
          },
          data: values,
        },
      ],
    };
  }

  getDiskChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.diskUsageSeries) {
      return {};
    }

    const data = this.resourceTrends.diskUsageSeries;
    const times = data.map(d => new Date(d.timestamp).toLocaleTimeString());
    const values = data.map(d => d.value);

    return {
      ...this.getBaseChartOptions('#fc6'),
      xAxis: {
        ...this.getBaseChartOptions('#fc6').xAxis,
        data: times,
      },
      yAxis: {
        type: 'value',
        max: 100,
      },
      series: [
        {
          name: 'Disk Usage (%)',
          type: 'line',
          smooth: true,
          symbol: 'circle',
          symbolSize: 6,
          sampling: 'lttb',
          itemStyle: {
            color: '#fc6',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(255, 204, 102, 0.3)' },
                { offset: 1, color: 'rgba(255, 204, 102, 0.05)' },
              ],
            },
          },
          data: values,
        },
      ],
    };
  }

  // Combined CPU/Memory/Disk Chart (三合一资源图表)
  getResourceChartOptions(): any {
    if (!this.resourceTrends || 
        !this.resourceTrends.cpuUsageSeries || 
        !this.resourceTrends.memoryUsageSeries || 
        !this.resourceTrends.diskUsageSeries) {
      return {};
    }

    const cpuData = this.resourceTrends.cpuUsageSeries;
    const memoryData = this.resourceTrends.memoryUsageSeries;
    const diskData = this.resourceTrends.diskUsageSeries;
    
    const times = cpuData.map(d => new Date(d.timestamp).toLocaleTimeString());
    const cpuValues = cpuData.map(d => d.value);
    const memoryValues = memoryData.map(d => d.value);
    const diskValues = diskData.map(d => d.value);

    return {
      ...this.getBaseChartOptions('#36f'),
      xAxis: {
        ...this.getBaseChartOptions('#36f').xAxis,
        data: times,
      },
      yAxis: {
        type: 'value',
        max: 100,
        axisLabel: {
          formatter: '{value}%',
        },
      },
      legend: {
        data: ['CPU', 'Memory', 'Disk'],
        top: 0,
      },
      tooltip: {
        trigger: 'axis',
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
            color: '#3366ff',
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
                { offset: 0, color: 'rgba(51, 102, 255, 0.2)' },
                { offset: 1, color: 'rgba(51, 102, 255, 0.02)' },
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
            color: '#ff6b6b',
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
                { offset: 0, color: 'rgba(255, 107, 107, 0.2)' },
                { offset: 1, color: 'rgba(255, 107, 107, 0.02)' },
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
            color: '#51cf66',
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
                { offset: 0, color: 'rgba(81, 207, 102, 0.2)' },
                { offset: 1, color: 'rgba(81, 207, 102, 0.02)' },
              ],
            },
          },
          data: diskValues,
        },
      ],
    };
  }

  getNetworkChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.networkTxSeries || !this.resourceTrends.networkRxSeries) {
      return {};
    }

    const txData = this.resourceTrends.networkTxSeries;
    const rxData = this.resourceTrends.networkRxSeries;
    const times = txData.map(d => new Date(d.timestamp).toLocaleTimeString());
    const txValues = txData.map(d => d.value / 1024 / 1024); // Convert to MB/s
    const rxValues = rxData.map(d => d.value / 1024 / 1024);

    return {
      ...this.getBaseChartOptions('#36f'),
      xAxis: {
        ...this.getBaseChartOptions('#36f').xAxis,
        data: times,
      },
      legend: {
        data: ['TX (Send)', 'RX (Receive)'],
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
            color: '#36f',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(51, 102, 255, 0.3)' },
                { offset: 1, color: 'rgba(51, 102, 255, 0.05)' },
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
            color: '#3c6',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(51, 204, 102, 0.3)' },
                { offset: 1, color: 'rgba(51, 204, 102, 0.05)' },
              ],
            },
          },
          data: rxValues,
        },
      ],
    };
  }

  getIoChartOptions(): any {
    if (!this.resourceTrends || !this.resourceTrends.ioReadSeries || !this.resourceTrends.ioWriteSeries) {
      return {};
    }

    const readData = this.resourceTrends.ioReadSeries;
    const writeData = this.resourceTrends.ioWriteSeries;
    const times = readData.map(d => new Date(d.timestamp).toLocaleTimeString());
    const readValues = readData.map(d => d.value / 1024 / 1024); // Convert to MB/s
    const writeValues = writeData.map(d => d.value / 1024 / 1024);

    return {
      ...this.getBaseChartOptions('#f90'),
      xAxis: {
        ...this.getBaseChartOptions('#f90').xAxis,
        data: times,
      },
      legend: {
        data: ['Read', 'Write'],
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
            color: '#3c6',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(51, 204, 102, 0.3)' },
                { offset: 1, color: 'rgba(51, 204, 102, 0.05)' },
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
            color: '#f90',
          },
          areaStyle: {
            color: {
              type: 'linear',
              x: 0,
              y: 0,
              x2: 0,
              y2: 1,
              colorStops: [
                { offset: 0, color: 'rgba(255, 153, 0, 0.3)' },
                { offset: 1, color: 'rgba(255, 153, 0, 0.05)' },
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

  // Mock data for frontend testing
  loadMockData() {
    // Generate time series data
    const now = new Date();
    const qpsSeries = [];
    const latencySeries = [];
    const errorRateSeries = [];
    const cpuSeries = [];
    const memorySeries = [];
    const diskSeries = [];
    const networkTxSeries = [];
    const networkRxSeries = [];
    const ioReadSeries = [];
    const ioWriteSeries = [];

    // Generate 60 data points
    for (let i = 60; i >= 0; i--) {
      const time = new Date(now.getTime() - i * 60 * 1000);
      const timestamp = time.toISOString();
      
      // Simulate realistic patterns
      qpsSeries.push({ timestamp, value: Math.floor(Math.random() * 500 + 800) }); // 800-1300 QPS
      latencySeries.push({ timestamp, value: Math.random() * 50 + 20 }); // 20-70ms
      errorRateSeries.push({ timestamp, value: Math.random() * 2 }); // 0-2%
      cpuSeries.push({ timestamp, value: Math.random() * 30 + 40 }); // 40-70%
      memorySeries.push({ timestamp, value: Math.random() * 20 + 60 }); // 60-80%
      diskSeries.push({ timestamp, value: Math.random() * 10 + 70 }); // 70-80%
      networkTxSeries.push({ timestamp, value: Math.random() * 50 + 100 }); // 100-150 MB/s
      networkRxSeries.push({ timestamp, value: Math.random() * 80 + 150 }); // 150-230 MB/s
      ioReadSeries.push({ timestamp, value: Math.random() * 200 + 300 }); // 300-500 MB/s
      ioWriteSeries.push({ timestamp, value: Math.random() * 150 + 200 }); // 200-350 MB/s
    }

    // Health Cards
    this.healthCards = [
      {
        title: 'QPS',
        value: '1,234',
        unit: '/s',
        trend: 5.2,
        status: 'success',
        icon: 'activity-outline',
        navigateTo: '/pages/starrocks/queries'
      },
      {
        title: 'P99 延迟',
        value: '45',
        unit: 'ms',
        trend: -2.3,
        status: 'success',
        icon: 'clock-outline'
      },
      {
        title: '错误率',
        value: '0.8',
        unit: '%',
        trend: 0.1,
        status: 'warning',
        icon: 'alert-triangle-outline'
      },
      {
        title: 'CPU',
        value: '55',
        unit: '%',
        trend: 3.5,
        status: 'info',
        icon: 'cpu-outline'
      },
      {
        title: '内存',
        value: '72',
        unit: '%',
        trend: 1.2,
        status: 'info',
        icon: 'inbox-outline'
      },
      {
        title: '磁盘',
        value: '75',
        unit: '%',
        trend: 2.8,
        status: 'warning',
        icon: 'hard-drive-outline'
      },
      {
        title: 'BE 节点',
        value: '8/8',
        unit: '',
        trend: 0,
        status: 'success',
        icon: 'server-outline',
        navigateTo: '/pages/starrocks/backends'
      },
      {
        title: 'FE 节点',
        value: '3/3',
        unit: '',
        trend: 0,
        status: 'success',
        icon: 'monitor-outline',
        navigateTo: '/pages/starrocks/frontends'
      },
      {
        title: '运行事务',
        value: '156',
        unit: '',
        trend: -5.6,
        status: 'info',
        icon: 'swap-outline'
      },
      {
        title: '数据增量',
        value: '2.3',
        unit: 'GB/天',
        trend: 12.5,
        status: 'info',
        icon: 'trending-up-outline'
      }
    ];

    // Performance Trends
    this.performanceTrends = {
      qpsSeries: qpsSeries,
      latencyP99Series: latencySeries,
      errorRateSeries: errorRateSeries
    };

    // Resource Trends
    this.resourceTrends = {
      cpuUsageSeries: cpuSeries,
      memoryUsageSeries: memorySeries,
      diskUsageSeries: diskSeries,
      networkTxSeries: networkTxSeries,
      networkRxSeries: networkRxSeries,
      ioReadSeries: ioReadSeries,
      ioWriteSeries: ioWriteSeries
    };

    // Data Statistics
    this.dataStatistics = {
      databaseCount: 12,
      tableCount: 456,
      totalDataSizeBytes: 1024 * 1024 * 1024 * 1024 * 5.6, // 5.6 TB
      activeUsers1h: 23,
      activeUsers24h: 156,
      mvTotal: 45,
      mvRunning: 3,
      mvSuccess: 40,
      mvFailed: 2,
      schemaChangeRunning: 2,
      schemaChangePending: 5,
      schemaChangeFinished: 234,
      schemaChangeFailed: 8,
      topTablesBySize: [
        { database: 'analytics', table: 'user_events', sizeBytes: 1024 * 1024 * 1024 * 890, rowCount: 25000000000 },
        { database: 'analytics', table: 'page_views', sizeBytes: 1024 * 1024 * 1024 * 650, rowCount: 18000000000 },
        { database: 'warehouse', table: 'orders', sizeBytes: 1024 * 1024 * 1024 * 420, rowCount: 8500000000 },
        { database: 'warehouse', table: 'order_items', sizeBytes: 1024 * 1024 * 1024 * 380, rowCount: 12000000000 },
        { database: 'analytics', table: 'sessions', sizeBytes: 1024 * 1024 * 1024 * 320, rowCount: 6000000000 },
        { database: 'logs', table: 'access_logs', sizeBytes: 1024 * 1024 * 1024 * 280, rowCount: 15000000000 },
        { database: 'warehouse', table: 'products', sizeBytes: 1024 * 1024 * 1024 * 180, rowCount: 2000000000 },
        { database: 'analytics', table: 'conversions', sizeBytes: 1024 * 1024 * 1024 * 120, rowCount: 1500000000 },
        { database: 'warehouse', table: 'customers', sizeBytes: 1024 * 1024 * 1024 * 95, rowCount: 800000000 },
        { database: 'logs', table: 'error_logs', sizeBytes: 1024 * 1024 * 1024 * 75, rowCount: 5000000000 }
      ],
      topTablesByAccess: [
        { database: 'warehouse', table: 'products', accessCount: 15680, uniqueUsers: 45, lastAccess: new Date(now.getTime() - 2 * 60 * 1000).toISOString() },
        { database: 'warehouse', table: 'orders', accessCount: 12340, uniqueUsers: 38, lastAccess: new Date(now.getTime() - 5 * 60 * 1000).toISOString() },
        { database: 'analytics', table: 'user_events', accessCount: 9876, uniqueUsers: 32, lastAccess: new Date(now.getTime() - 8 * 60 * 1000).toISOString() },
        { database: 'warehouse', table: 'customers', accessCount: 8765, uniqueUsers: 28, lastAccess: new Date(now.getTime() - 12 * 60 * 1000).toISOString() },
        { database: 'analytics', table: 'page_views', accessCount: 7654, uniqueUsers: 25, lastAccess: new Date(now.getTime() - 15 * 60 * 1000).toISOString() },
        { database: 'warehouse', table: 'order_items', accessCount: 6543, uniqueUsers: 22, lastAccess: new Date(now.getTime() - 20 * 60 * 1000).toISOString() },
        { database: 'analytics', table: 'sessions', accessCount: 5432, uniqueUsers: 19, lastAccess: new Date(now.getTime() - 25 * 60 * 1000).toISOString() },
        { database: 'logs', table: 'access_logs', accessCount: 4321, uniqueUsers: 15, lastAccess: new Date(now.getTime() - 35 * 60 * 1000).toISOString() },
        { database: 'analytics', table: 'conversions', accessCount: 3210, uniqueUsers: 12, lastAccess: new Date(now.getTime() - 45 * 60 * 1000).toISOString() },
        { database: 'logs', table: 'error_logs', accessCount: 2109, uniqueUsers: 8, lastAccess: new Date(now.getTime() - 60 * 60 * 1000).toISOString() }
      ],
      slowQueries: [
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'analyst_user', 
          database: 'analytics', 
          durationMs: 45600, 
          scanRows: 250000000, 
          scanBytes: 1024 * 1024 * 1024 * 15, 
          returnRows: 50000, 
          cpuCostMs: 42000, 
          memCostBytes: 1024 * 1024 * 512, 
          timestamp: new Date(now.getTime() - 15 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT COUNT(*) FROM user_events WHERE date >= \'2024-01-01\' GROUP BY user_id' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'data_engineer', 
          database: 'warehouse', 
          durationMs: 38900, 
          scanRows: 180000000, 
          scanBytes: 1024 * 1024 * 1024 * 12, 
          returnRows: 120000, 
          cpuCostMs: 35000, 
          memCostBytes: 1024 * 1024 * 768, 
          timestamp: new Date(now.getTime() - 25 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT o.*, oi.* FROM orders o JOIN order_items oi ON o.order_id = oi.order_id WHERE o.status = \'pending\'' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'analyst_user', 
          database: 'analytics', 
          durationMs: 32100, 
          scanRows: 150000000, 
          scanBytes: 1024 * 1024 * 1024 * 10, 
          returnRows: 8000, 
          cpuCostMs: 29000, 
          memCostBytes: 1024 * 1024 * 384, 
          timestamp: new Date(now.getTime() - 35 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT session_id, COUNT(*) FROM page_views GROUP BY session_id HAVING COUNT(*) > 100' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'report_user', 
          database: 'warehouse', 
          durationMs: 28700, 
          scanRows: 120000000, 
          scanBytes: 1024 * 1024 * 1024 * 8, 
          returnRows: 5000, 
          cpuCostMs: 25000, 
          memCostBytes: 1024 * 1024 * 512, 
          timestamp: new Date(now.getTime() - 45 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT p.*, SUM(oi.quantity) FROM products p JOIN order_items oi ON p.product_id = oi.product_id GROUP BY p.product_id' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'data_engineer', 
          database: 'logs', 
          durationMs: 25400, 
          scanRows: 200000000, 
          scanBytes: 1024 * 1024 * 1024 * 18, 
          returnRows: 150000, 
          cpuCostMs: 22000, 
          memCostBytes: 1024 * 1024 * 256, 
          timestamp: new Date(now.getTime() - 55 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT * FROM access_logs WHERE timestamp > DATE_SUB(NOW(), INTERVAL 30 DAY) ORDER BY timestamp DESC' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'analyst_user', 
          database: 'analytics', 
          durationMs: 22800, 
          scanRows: 95000000, 
          scanBytes: 1024 * 1024 * 1024 * 6, 
          returnRows: 90, 
          cpuCostMs: 20000, 
          memCostBytes: 1024 * 1024 * 384, 
          timestamp: new Date(now.getTime() - 65 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT date, COUNT(DISTINCT user_id) FROM user_events GROUP BY date ORDER BY date DESC LIMIT 90' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'report_user', 
          database: 'warehouse', 
          durationMs: 19500, 
          scanRows: 85000000, 
          scanBytes: 1024 * 1024 * 1024 * 5, 
          returnRows: 12000, 
          cpuCostMs: 17000, 
          memCostBytes: 1024 * 1024 * 512, 
          timestamp: new Date(now.getTime() - 75 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT c.customer_id, c.name, COUNT(o.order_id) FROM customers c LEFT JOIN orders o ON c.customer_id = o.customer_id GROUP BY c.customer_id' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'data_engineer', 
          database: 'analytics', 
          durationMs: 17200, 
          scanRows: 65000000, 
          scanBytes: 1024 * 1024 * 1024 * 4, 
          returnRows: 3500, 
          cpuCostMs: 15000, 
          memCostBytes: 1024 * 1024 * 256, 
          timestamp: new Date(now.getTime() - 85 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT * FROM sessions WHERE duration > 3600 AND page_count > 50' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'analyst_user', 
          database: 'warehouse', 
          durationMs: 15600, 
          scanRows: 52000000, 
          scanBytes: 1024 * 1024 * 1024 * 3, 
          returnRows: 100, 
          cpuCostMs: 13000, 
          memCostBytes: 1024 * 1024 * 256, 
          timestamp: new Date(now.getTime() - 95 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT product_id, SUM(quantity * price) as revenue FROM order_items GROUP BY product_id ORDER BY revenue DESC LIMIT 100' 
        },
        { 
          queryId: 'q-' + Math.random().toString(36).substr(2, 9), 
          user: 'report_user', 
          database: 'logs', 
          durationMs: 13800, 
          scanRows: 48000000, 
          scanBytes: 1024 * 1024 * 1024 * 2, 
          returnRows: 850, 
          cpuCostMs: 12000, 
          memCostBytes: 1024 * 1024 * 128, 
          timestamp: new Date(now.getTime() - 105 * 60 * 1000).toISOString(), 
          state: 'FINISHED', 
          queryPreview: 'SELECT error_type, COUNT(*) FROM error_logs WHERE severity = \'ERROR\' GROUP BY error_type' 
        }
      ]
    };

    // Capacity Prediction
    this.capacityPrediction = {
      diskUsagePct: 75.2,
      diskUsedBytes: 1024 * 1024 * 1024 * 1024 * 6.2, // 6.2 TB
      diskTotalBytes: 1024 * 1024 * 1024 * 1024 * 8.0, // 8 TB
      dailyGrowthBytes: 1024 * 1024 * 1024 * 2.3, // 2.3 GB/day
      daysUntilFull: 285,
      predictedFullDate: '2025-08-01',
      growthTrend: 'stable'
    };
  }
}


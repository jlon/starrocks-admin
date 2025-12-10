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
  
  // LLM Analysis state (loaded async after DAG)
  llmAnalysisLoading = false;
  llmAnalysisError: string = '';
  llmAnalysisStartTime: number = 0;
  llmAnalysisElapsedTime: number = 0;
  
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
  isRootCauseCollapsed = false; // Root cause analysis section
  showMemoryView = false; // Toggle between Time and Memory view
  
  // Graph direction: 'BT' (bottom-to-top) or 'LR' (left-to-right)
  graphDirection: 'BT' | 'LR' = 'BT';
  
  // Data flow animation (enabled by default)
  isFlowAnimationEnabled = true;
  
  // Node detail view state (when a node is selected)
  selectedNodeTab: 'core' | 'detail' | 'pipeline' = 'core';
  
  // Core metrics by operator type
  private coreMetricsByType: { [key: string]: string[] } = {
    'SCAN': ['RawRowsRead', 'BytesRead', 'CompressedBytesRead', 'ScanTime', 'IOTime', 'Table', 'Rollup', 'TabletCount', 'PushdownPredicates', 'BitmapIndexFilterRows', 'BloomFilterFilterRows', 'RuntimeInFilterRows', 'RuntimeBloomFilterRows'],
    'JOIN': ['BuildRows', 'ProbeRows', 'HashTableMemoryUsage', 'RuntimeFilterNum', 'JoinType', 'DistributionMode', 'HashTableSize', 'ProbeTime', 'BuildTime'],
    'AGGREGATE': ['InputRowCount', 'OutputRowCount', 'HashTableMemoryUsage', 'AggFuncComputeTime', 'GroupByKeyCount', 'StreamingTime', 'HashTableIteratorTime'],
    'EXCHANGE': ['ShuffleRowsPerBuffer', 'NetworkTime', 'WaitTime', 'OverallThroughput', 'PeakBufferUsage'],
    'SORT': ['SortKeys', 'SortType', 'TopNLimit', 'SortTime', 'MergeTime'],
    'PROJECT': ['ExprComputeTime', 'CommonSubExprComputeTime'],
    'DEFAULT': ['OperatorTotalTime', 'PullRowNum', 'PushRowNum', 'PeakMemoryUsage']
  };
  
  private nodeRankMap: Map<string, number> = new Map(); // Node rank by time percentage
  objectKeys = Object.keys; // Helper for template

  // Metric descriptions for tooltips
  // Reference: https://docs.starrocks.io/zh/docs/best_practices/query_tuning/query_profile_operator_metrics/
  metricDescriptions: { [key: string]: string } = {
    // ============================================
    // 概要指标 (Summary Metrics)
    // ============================================
    'Total': '查询消耗的总时间，包括计划、执行和分析阶段的持续时间',
    'QueryState': '查询状态，可能的状态包括Finished、Error和Running',
    'QueryId': '查询的唯一标识符',
    'StartTime': '查询开始的时间戳',
    'EndTime': '查询结束的时间戳',
    'QueryType': '查询的类型',
    'StarRocksVersion': '使用的StarRocks版本',
    'User': '执行查询的用户',
    'DefaultDb': '查询使用的默认数据库',
    'SqlStatement': '执行的SQL语句',
    'Variables': '查询中使用的重要变量',
    'NonDefaultSessionVariables': '查询中使用的非默认会话变量',
    'CollectProfileTime': '收集概要所花费的时间',
    'IsProfileAsync': '指示概要收集是否为异步',

    // ============================================
    // 执行概览指标 (Execution Overview Metrics)
    // ============================================
    'FrontendProfileMergeTime': 'FE端概要处理时间',
    'QueryAllocatedMemoryUsage': '节点间分配的总内存',
    'QueryDeallocatedMemoryUsage': '节点间释放的总内存',
    'QueryPeakMemoryUsagePerNode': '每个节点的最大内存峰值',
    'QuerySumMemoryUsage': '节点间的总内存峰值',
    'QueryExecutionWallTime': '执行的墙钟时间',
    'ExecutionWallTime': 'Query执行总耗时',
    'QueryCumulativeCpuTime': '节点间的总CPU时间',
    'QueryCpuTime': '查询累积CPU使用时间，所有并发进程累加，因此该指标会大于实际的执行时间',
    'QueryCumulativeOperatorTime': 'Operator执行的总时间，为Operator时间百分比的分母',
    'OperatorTime': '所有Operator累计执行时间',
    'QueryCumulativeNetworkTime': 'Exchange节点的总网络时间',
    'QueryNetworkTime': '所有Exchange节点网络传输耗时之和',
    'QueryCumulativeScanTime': 'Scan节点的总IO时间',
    'QueryScanTime': '所有SCAN节点扫描耗时之和',
    'QueryPeakScheduleTime': '最大Pipeline调度时间',
    'PeakScheduleTime': 'Pipeline调度峰值等待时间',
    'QuerySpillBytes': '溢出到磁盘的数据',
    'SpillBytes': '溢出到磁盘的数据量',
    'ResultDeliverTime': '结果传输耗时',
    'MemoryUsage': '查询内存使用量',

    // ============================================
    // Fragment 指标
    // ============================================
    'InstanceNum': 'FragmentInstances的数量',
    'InstanceIds': '所有FragmentInstances的ID',
    'BackendNum': '参与的BE数量',
    'BackendAddresses': 'BE地址',
    'FragmentInstancePrepareTime': 'Fragment准备阶段的持续时间',
    'InstanceAllocatedMemoryUsage': '实例分配的总内存',
    'InstanceDeallocatedMemoryUsage': '实例释放的总内存',
    'InstancePeakMemoryUsage': '实例间的内存峰值',
    'FragmentID': 'Fragment标识符',

    // ============================================
    // Pipeline 指标
    // ============================================
    'DriverTotalTime': 'Driver消耗的总时间。DriverTotalTime = ActiveTime + PendingTime + ScheduleTime',
    'ActiveTime': 'Driver执行时间。ActiveTime = ∑OperatorTotalTime + OverheadTime',
    'PendingTime': 'Driver因为输入或者前置条件不满足等待的时间',
    'ScheduleTime': 'Driver调度等待时间',
    'InputEmptyTime': '输入为空的等待时间',
    'FirstInputEmptyTime': '首次输入为空的等待时间',
    'OutputFullTime': '输出满的等待时间',
    'PreconditionBlockTime': '前置条件阻塞时间',
    'PipelineID': 'Pipeline标识符',
    'Depth': '节点在执行树中的深度',
    'NodeType': '节点类型',
    'IsHotspot': '是否为热点节点',
    'HotspotSeverity': '热点严重程度',
    'IsGroupExecution': '是否为分组执行',
    'BlockByInputEmpty': '因输入为空而阻塞的次数',
    'BlockByOutputFull': '因输出满而阻塞的次数',
    'BlockByPrecondition': '因前置条件不满足而阻塞的次数',
    'DegreeOfParallelism': '并行度',
    'TotalDegreeOfParallelism': '总并行度',
    'PeakDriverQueueSize': 'Driver队列峰值大小',
    'ScheduleCount': '调度次数',
    'YieldByLocalWait': '因本地等待而让出的次数',
    'YieldByPreempt': '因抢占而让出的次数',
    'YieldByTimeLimit': '因时间限制而让出的次数',
    'EnableEventScheduler': '是否启用事件调度器',
    'InitialProcessDriverCount': '初始处理Driver数量',
    'InitialProcessMem': '初始处理内存',
    'JITCounter': 'JIT编译计数',
    'JITTotalCostTime': 'JIT编译总耗时',
    'QueryMemoryLimit': '查询内存限制',
    'BackendProfileMergeTime': 'BE端Profile合并时间',

    // ============================================
    // Operator 通用指标
    // ============================================
    'OperatorTotalTime': 'Operator消耗的总时间',
    'PushRowNum': 'Operator累积输出行数',
    'PullRowNum': 'Operator累积输入行数',
    'PullChunkNum': 'Operator累积输入Chunk数',
    'PushChunkNum': 'Operator累积输出Chunk数',
    'PullTotalTime': 'Pull操作总耗时',
    'PushTotalTime': 'Push操作总耗时',
    'PeakMemoryUsage': 'Operator最大内存使用量',
    'OutputRows': 'Operator输出行数',
    'TimePercentage': '该Operator耗时占总耗时的百分比',
    'OutputChunkBytes': 'Operator输出Chunk的字节数',
    'IsSubordinate': '是否为从属Operator',
    'ConjunctsInputRows': '谓词输入行数',
    'ConjunctsOutputRows': '谓词输出行数',
    'ConjunctsTime': '谓词计算耗时',
    'RuntimeFilterNum': 'Runtime Filter数量',
    'RuntimeInFilterNum': 'Runtime In Filter数量',
    'JoinRuntimeFilterEvaluate': 'Join Runtime Filter评估次数',
    'JoinRuntimeFilterHashTime': 'Join Runtime Filter哈希计算耗时',
    'JoinRuntimeFilterInputRows': 'Join Runtime Filter输入行数',
    'JoinRuntimeFilterOutputRows': 'Join Runtime Filter输出行数',
    'JoinRuntimeFilterTime': 'Join Runtime Filter耗时',
    'SetFinishingTime': '设置完成状态耗时',
    'RequestReceived': '接收的请求数',
    'SinkType': 'Sink类型',
    'AppendChunkTime': '追加Chunk耗时',
    'ResultRendTime': '结果渲染耗时',
    'TupleConvertTime': '元组转换耗时',
    'NumSentRows': '发送的行数',

    // ============================================
    // Scan Operator (OLAP/Connector)
    // ============================================
    'Table': '表名称',
    'Rollup': '使用的物化视图或Rollup名称',
    'ScanTime': 'Scan累计时间，Scan操作在异步I/O线程池中完成',
    'TabletCount': 'Tablet数量',
    'MorselsCount': 'Morsel数量',
    'PushdownPredicates': '下推的谓词数量',
    'BytesRead': '读取数据的大小',
    'RowsRead': '读取的行数',
    'RawRowsRead': '实际扫描到的原始记录行数',
    'CompressedBytesRead': '读取压缩数据的大小',
    'UncompressedBytesRead': '读取解压后数据的大小',
    'IOTime': '累计I/O时间',
    'IOTimeTotal': '总I/O时间',
    'IOTimeLocalDisk': '从本地缓存读取数据时所产生的I/O操作耗时',
    'IOTimeRemote': '从OSS读取数据时所产生的I/O操作耗时',
    'BitmapIndexFilterRows': 'Bitmap索引过滤的数据行数',
    'BitmapIndexFilter': 'Bitmap索引过滤耗时',
    'BloomFilterFilterRows': 'Bloomfilter过滤的数据行数',
    'BloomFilterFilter': 'Bloomfilter过滤耗时',
    'SegmentRuntimeZoneMapFilterRows': 'Runtime Zone Map过滤的数据行数',
    'SegmentZoneMapFilterRows': 'Zone Map过滤的数据行数',
    'ZoneMapIndexFilterRows': 'Zone Map索引过滤的数据行数',
    'ZoneMapIndexFiter': 'Zone Map索引过滤耗时',
    'ShortKeyFilterRows': 'Short Key过滤的数据行数',
    'ShortKeyFilter': 'Short Key过滤耗时',
    'PredFilter': '谓词过滤耗时',
    'PredFilterRows': '谓词过滤的行数',
    'DelVecFilterRows': '删除向量过滤的行数',
    'SegmentInit': 'Segment初始化耗时',
    'SegmentRead': 'Segment读取耗时',
    'ColumnIteratorInit': '列迭代器初始化耗时',
    'BlockSeek': 'Block寻址耗时',
    'BlockSeekCount': 'Block寻址次数',
    'BlockFetch': 'Block获取耗时',
    'BlockFetchCount': 'Block获取次数',
    'ChunkCopy': 'Chunk复制耗时',
    'DecompressT': '解压耗时',
    'CreateSegmentIter': '创建Segment迭代器耗时',
    'GetDelVec': '获取删除向量耗时',
    'GetDeltaColumnGroup': '获取Delta列组耗时',
    'PrepareChunkSourceTime': '准备Chunk源耗时',
    'SubmitTaskTime': '提交任务耗时',
    'SubmitTaskCount': '提交任务次数',
    'IOTaskWaitTime': 'IO任务等待时间',
    'IOTaskExecTime': 'IO任务执行时间',
    'PrefetchHitCount': '预取命中次数',
    'PrefetchPendingTime': '预取等待时间',
    'PrefetchWaitFinishTime': '预取完成等待时间',
    
    // Connector Scan (存算分离)
    'CompressedBytesReadLocalDisk': '从计算节点的本地缓存读取的经过压缩后的数据量',
    'CompressedBytesReadRemote': '从OSS读取的经过压缩的数据总量',
    'CompressedBytesReadRequest': '请求读取的压缩数据大小',
    'CompressedBytesReadTotal': '读取的压缩数据总量',
    'IOCountLocalDisk': '本地磁盘IO次数',
    'IOCountRemote': '远程IO次数',
    'IOCountRequest': '请求IO次数',
    'IOCountTotal': '总IO次数',
    'PagesCountLocalDisk': '本地磁盘页数',
    'PagesCountRemote': '远程页数',
    'PagesCountMemory': '内存页数',
    'PagesCountTotal': '总页数',
    'DataSourceType': '数据源类型',
    'SharedScan': '是否共享扫描',
    'MorselQueueType': 'Morsel队列类型',
    'AdaptiveIOTasks': '是否启用自适应IO任务',
    'PeakIOTasks': 'IO任务峰值数',
    'ChunkBufferCapacity': 'Chunk缓冲区容量',
    'DefaultChunkBufferCapacity': '默认Chunk缓冲区容量',
    'PeakChunkBufferSize': 'Chunk缓冲区峰值大小',
    'PeakChunkBufferMemoryUsage': 'Chunk缓冲区内存使用峰值',
    'PeakScanTaskQueueSize': '扫描任务队列峰值大小',
    'RowsetsReadCount': '读取的Rowset数量',
    'SegmentsReadCount': '读取的Segment数量',
    'TotalColumnsDataPageCount': '总列数据页数',
    'AccessPathHits': '访问路径命中次数',
    'AccessPathUnhits': '访问路径未命中次数',
    'PushdownAccessPaths': '下推的访问路径数',
    'ShortKeyRangeNumber': 'Short Key范围数',
    'RemainingRowsAfterShortKeyFilter': 'Short Key过滤后剩余行数',
    'MemAllocFailed': '内存分配失败次数',
    'IOStatistics': 'IO统计信息',
    'ReadPKIndex': '读取主键索引耗时',
    'BitmapIndexIteratorInit': 'Bitmap索引迭代器初始化耗时',
    'RawInputBytes': '原始输入字节数',

    // ============================================
    // Exchange Operator
    // ============================================
    // Sink
    'PartType': '数据分布模式，包括UNPARTITIONED、RANDOM、HASH_PARTITIONED和BUCKET_SHUFFLE_HASH_PARTITIONED',
    'DestFragments': '目标Fragment',
    'DestID': '目标ID',
    'ChannelNum': '通道数量',
    'BytesSent': '发送的数据大小',
    'BytesUnsent': '未发送的数据大小',
    'RequestSent': '已发送请求数',
    'RequestUnsent': '未发送请求数',
    'BytesPassThrough': 'PassThrough的字节数',
    'PassThroughBufferPeakMemoryUsage': 'PassThrough缓冲区内存使用峰值',
    'OverallThroughput': '吞吐速率',
    'OverallTime': '总体时间',
    'NetworkTime': '数据包传输时间（不包括接收后处理时间）',
    'WaitTime': '由于发送端队列满而导致的等待时间',
    'NetworkBandwidth': '网络带宽',
    'SerializeChunkTime': '序列化Chunk耗时',
    'SerializedBytes': '序列化字节数',
    'CompressTime': '压缩耗时',
    'CompressedBytes': '压缩后字节数',
    'RpcCount': 'RPC调用次数',
    'RpcAvgTime': 'RPC平均耗时',
    'ShuffleHashTime': 'Shuffle哈希计算耗时',
    'ShuffleChunkAppendTime': 'Shuffle Chunk追加耗时',
    'ShuffleChunkAppendCounter': 'Shuffle Chunk追加计数',
    'BufferUnplugCount': '缓冲区解除阻塞次数',
    'ClosureBlockCount': '闭包阻塞次数',
    'ClosureBlockTime': '闭包阻塞时间',
    
    // Source
    'SenderWaitLockTime': '等锁时间',
    'WaitLockTime': '等锁时间',
    'BytesReceived': '接收的数据大小',
    'DecompressChunkTime': '解压时间',
    'DeserializeChunkTime': '反序列化时间',
    'SenderTotalTime': '发送总时间',
    'ReceiverProcessTotalTime': '接收端处理总时间',
    'PeakBufferMemoryBytes': '缓冲区内存峰值字节数',

    // ============================================
    // Aggregate Operator
    // ============================================
    'GroupingKeys': 'GROUP BY列',
    'AggregateFunctions': '聚合函数',
    'AggComputeTime': '聚合函数计算耗时',
    'ExprComputeTime': '表达式计算耗时',
    'HashTableSize': 'Hash Table大小',
    'HashTableMemoryUsage': 'Hash Table内存使用量',
    'InputRowCount': '输入行数',
    'PassThroughRowCount': 'PassThrough行数',
    'ResultAggAppendTime': '结果聚合追加耗时',
    'ResultGroupByAppendTime': '结果分组追加耗时',
    'GetResultsTime': '获取结果耗时',
    'IteratorMergeTime': '迭代器合并耗时',
    'StreamingTime': '流式处理耗时',
    'AggStateName': '聚合状态名称',

    // ============================================
    // Join Operator
    // ============================================
    // Probe
    'DistributionMode': '数据分布模式',
    'JoinType': 'Join类型',
    'OtherJoinConjunctEvaluateTime': '其他JoinConjunct耗时',
    'ProbeConjunctEvaluateTime': 'Probe Conjunct耗时',
    'SearchHashTableTime': '查询Hash Table耗时',
    'SearchHashTableCount': '查询Hash Table次数',
    'WhereConjunctEvaluateTime': 'Where Conjunct耗时',
    'OutputBuildColumnTime': '输出Build列耗时',
    'OutputProbeColumnTime': '输出Probe列耗时',
    'OutputTupleColumnTime': '输出Tuple列耗时',
    'ProbeRowsCounter': 'Probe行计数',
    
    // Build
    'JoinPredicates': 'Join谓词',
    'BuildBuckets': 'Hash Table的Bucket数量',
    'BuildHashTableTime': '构建Hash Table耗时',
    'CopyRightTableChunkTime': '复制右表Chunk耗时',
    'RuntimeFilterBuildTime': 'Runtime Filter构建时间',
    'BuildConjunctEvaluateTime': 'Build Conjunct评估耗时',
    'BuildRowsCounter': 'Build行计数',
    'RuntimeBloomFilterNum': 'Runtime Bloom Filter个数',

    // ============================================
    // Window Function Operator
    // ============================================
    'ComputeTime': '窗口函数计算耗时',
    'PartitionKeys': '分区列',
    'PartitionChunksNum': '分区Chunk数',
    'PartitionRowsNum': '分区行数',
    'PeerGroupChunksNum': 'Peer Group Chunk数',
    'PeerGroupRowsNum': 'Peer Group行数',
    'ColumnResize': '列调整大小次数',

    // ============================================
    // Sort Operator
    // ============================================
    'SortKeys': '排序键',
    'SortType': '查询结果排序方式：全排序或者排序Top N个结果',
    'MaxBufferedRows': '缓冲行数的峰值',
    'MaxBufferedBytes': '缓冲字节数的峰值',
    'NumSortedRuns': '排序运行的次数',
    'BuildingTime': '排序期间维护内部数据结构所花费的时间',
    'MergingTime': '排序期间合并排序运行所花费的时间',
    'SortingTime': '排序所花费的时间',
    'OutputTime': '构建输出排序序列所花费的时间',

    // ============================================
    // Merge Operator
    // ============================================
    'Limit': '限制返回的行数',
    'Offset': '偏移量',
    'StreamingBatchSize': '当在流模式下执行合并时，每次合并操作处理的数据大小',
    'LateMaterializationMaxBufferChunkNum': '启用延迟物化时缓冲区中的最大Chunk数量',
    'OverallStageCount': '所有阶段的总执行次数',
    'OverallStageTime': '每个阶段的总执行时间',
    'LateMaterializationGenerateOrdinalTime': '延迟物化期间生成序数列所花费的时间',
    'SortedRunProviderTime': '在Process阶段从提供者检索数据所花费的时间',

    // ============================================
    // TableFunction Operator
    // ============================================
    'TableFunctionExecTime': 'Table Function计算耗时',
    'TableFunctionExecCount': 'Table Function执行次数',

    // ============================================
    // Project Operator
    // ============================================
    'CommonSubExprComputeTime': '公共子表达式计算耗时',

    // ============================================
    // LocalExchange Operator
    // ============================================
    'Type': 'Local Exchange类型，包括：Passthrough、Partition和Broadcast',
    'ShuffleNum': 'Shuffle数量，该指标仅当Type为Partition时有效',
    'LocalExchangePeakMemoryUsage': '内存使用峰值',
    'LocalExchangePeakBufferSize': '缓冲区的大小峰值',
    'LocalExchangePeakBufferMemoryUsage': '缓冲区的内存使用峰值',
    'LocalExchangePeakBufferChunkNum': '缓冲区中的Chunk数量峰值',
    'LocalExchangePeakBufferRowNum': '缓冲区中的行数峰值',
    'LocalExchangePeakBufferBytes': '缓冲区中的数据大小峰值',
    'LocalExchangePeakBufferChunkSize': '缓冲区中的Chunk大小峰值',
    'LocalExchangePeakBufferChunkRowNum': '缓冲区中每个Chunk的行数峰值',
    'LocalExchangePeakBufferChunkBytes': '缓冲区中每个Chunk的数据大小峰值',

    // ============================================
    // OlapTableSink Operator
    // ============================================
    'IndexNum': '为目标表创建的同步物化视图的数量',
    'ReplicatedStorage': '是否启用了单领导者复制',
    'TxnID': '导入事务的ID',
    'RowsFiltered': '由于数据质量不足而被过滤掉的行数',
    'RowsReturned': '写入目标表的行数',
    'RpcClientSideTime': '客户端记录的导入的总RPC时间消耗',
    'RpcServerSideTime': '服务器端记录的导入的总RPC时间消耗',
    'PrepareDataTime': '数据准备阶段的总时间消耗，包括数据格式转换和数据质量检查',
    'SendDataTime': '发送数据的本地时间消耗，包括序列化和压缩数据的时间，以及将任务提交到发送者队列的时间',

    // ============================================
    // 执行时间分类 (Execution Time Categories)
    // ============================================
    'IO': '所有SCAN节点IO耗时之和',
    'Processing': 'Operator节点用于记录其执行计算操作的总耗时',
    'IoSeekTime': 'IO Seek寻址过程产生的总耗时，该指标仅适用于存算分离实例',
    'LocalDiskReadIOTime': '从本地缓存读取数据产生的I/O耗时，该指标仅适用于存算分离实例',
    'RemoteReadIOTime': '从远端OSS读取数据产生的I/O耗时，该指标仅适用于存算分离实例',

  };

  // Get metric description (case-insensitive matching)
  getMetricDescription(key: string): string | null {
    // Direct match first
    if (this.metricDescriptions[key]) {
      return this.metricDescriptions[key];
    }
    // Case-insensitive match
    const lowerKey = key.toLowerCase();
    for (const [k, v] of Object.entries(this.metricDescriptions)) {
      if (k.toLowerCase() === lowerKey) {
        return v;
      }
    }
    return null;
  }

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
    
    // Load analysis data (includes profile_content)
    this.loadAnalysis(queryId);
  }
  
  // Load profile analysis for DAG (includes profile_content)
  loadAnalysis(queryId: string): void {
    this.analysisLoading = true;
    this.profileDetailLoading = true;
    this.analysisError = '';
    
    this.nodeService.analyzeProfile(queryId).subscribe({
      next: (data) => {
        this.analysisData = data;
        this.topNodes = data.summary?.top_time_consuming_nodes || [];
        // Set profile content from analysis response
        if (data.profile_content) {
          this.currentProfileDetail = data.profile_content;
        }
        if (data.execution_tree) {
          this.buildGraph(data.execution_tree);
        }
        this.analysisLoading = false;
        this.profileDetailLoading = false;
        
        // If LLM is available but pending, load it async
        if (data.llm_analysis?.available && data.llm_analysis?.status === 'pending') {
          this.loadLLMAnalysis(queryId);
        }
      },
      error: (err) => {
        console.error('Failed to analyze profile', err);
        this.analysisError = '分析失败: ' + (err.error?.message || err.message || '未知错误');
        this.analysisLoading = false;
        this.profileDetailLoading = false;
      }
    });
  }
  
  /**
   * Load LLM analysis async (called after DAG is rendered)
   * Passes pre-analyzed data to avoid redundant profile parsing
   */
  private loadLLMAnalysis(queryId: string, forceRefresh: boolean = false): void {
    if (!this.clusterId || !this.analysisData) return;
    
    this.llmAnalysisLoading = true;
    this.llmAnalysisError = '';
    this.llmAnalysisStartTime = Date.now();
    this.llmAnalysisElapsedTime = 0;
    
    // Pass pre-analyzed data to backend to avoid re-fetching profile
    // Build payload with analysis_data and force_refresh flag
    const payload = {
      analysis_data: this.analysisData,
      force_refresh: forceRefresh || false,
    };
    
    this.nodeService.enhanceProfileWithLLM(this.clusterId, queryId, payload).subscribe({
      next: (llmData) => {
        // Calculate elapsed time
        this.llmAnalysisElapsedTime = Date.now() - this.llmAnalysisStartTime;
        
        // Clear previous error
        this.llmAnalysisError = null;
        
        // Merge LLM analysis into existing data
        if (this.analysisData) {
          this.analysisData.llm_analysis = llmData;
          // Store elapsed time in analysis data
          if (this.analysisData.llm_analysis) {
            this.analysisData.llm_analysis.elapsed_time_ms = this.llmAnalysisElapsedTime;
          }
        }
        this.llmAnalysisLoading = false;
        this.cdr.detectChanges();
      },
      error: (err) => {
        console.error('Failed to load LLM analysis', err);
        this.llmAnalysisElapsedTime = Date.now() - this.llmAnalysisStartTime;
        this.llmAnalysisError = err.error?.message || 'LLM 分析加载失败';
        this.llmAnalysisLoading = false;
        // Update llm_analysis status
        if (this.analysisData?.llm_analysis) {
          this.analysisData.llm_analysis.status = 'failed';
          this.analysisData.llm_analysis.available = false;
        }
        this.cdr.detectChanges();
      }
    });
  }
  
  /**
   * Retry LLM analysis (force refresh)
   */
  retryLLMAnalysis(): void {
    if (this.currentQueryId) {
      this.loadLLMAnalysis(this.currentQueryId, true);
    }
  }
  
  /**
   * Get LLM analysis elapsed time in formatted string
   */
  getLLMElapsedTime(): string {
    const llmData = this.analysisData?.llm_analysis;
    const elapsedMs = llmData?.elapsed_time_ms || this.llmAnalysisElapsedTime;
    
    if (!elapsedMs) return '';
    
    if (elapsedMs < 1000) {
      return `${elapsedMs}ms`;
    } else {
      return `${(elapsedMs / 1000).toFixed(1)}s`;
    }
  }
  
  /**
   * Check if LLM analysis was cached
   */
  isLLMAnalysisCached(): boolean {
    return this.analysisData?.llm_analysis?.from_cache === true;
  }
  
  // Refresh analysis
  refreshAnalysis(): void {
    if (this.currentQueryId) {
      this.loadAnalysis(this.currentQueryId);
    }
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
      rankdir: this.graphDirection,
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

    // Extract coordinates - round to integer pixels to prevent subpixel blur
    this.graphNodes = nodeList.map((node: any) => {
      const layoutNode = g.node(node.id);
      
      // Calculate coordinates for sanitized output
      // Round to integers to avoid subpixel rendering blur
      const x = (layoutNode && isFinite(layoutNode.x)) ? Math.round(layoutNode.x) : 0;
      const y = (layoutNode && isFinite(layoutNode.y)) ? Math.round(layoutNode.y) : 0;
      const width = (layoutNode && isFinite(layoutNode.width)) ? Math.round(layoutNode.width) : 220;
      const height = (layoutNode && isFinite(layoutNode.height)) ? Math.round(layoutNode.height) : 95;

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
      const minStrokeWidth = 2;
      const maxStrokeWidth = 9;
      
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

      // Determine stroke color based on target node type
      let strokeColor = '#bfbfbf';
      if (targetNode) {
        const name = targetNode.operator_name?.toUpperCase() || '';
        if (name.includes('SCAN') || name.includes('JOIN')) {
          strokeColor = '#fa8c16';
        }
      }

      // Calculate arrow size based on stroke width
      // Arrow must be larger than stroke width to avoid being covered
      const arrowSize = Math.max(12, strokeWidth * 2.5);
      const arrowRefY = arrowSize / 2;
      // Polygon points: triangle pointing right - (0,0) -> (arrowSize, arrowSize/2) -> (0, arrowSize)
      const arrowPoints = `0,0 ${arrowSize},${arrowRefY} 0,${arrowSize}`;
      // refX = 0: arrow BASE aligns to path end, arrow TIP extends to original target
      const arrowRefX = 0;

      // Build edge path: For multi-child parents, use vertical-then-diagonal pattern (like Figure 2)
      // Path: childTop -> verticalLift -> parentBottom (3 points: vertical segment + diagonal segment)
      const hasMultipleChildren = targetNode?.children && targetNode.children.length >= 2;
      let displayPoints: { x: number, y: number }[] = [visibleStart, visibleEnd];

      if (hasMultipleChildren) {
        // Calculate vertical lift distance (how far to go up before turning diagonal)
        const deltaY = Math.abs(visibleStart.y - visibleEnd.y);
        const liftDistance = Math.min(50, deltaY * 0.35); // 35% of vertical distance, max 50px
        
        // Middle point: directly above source node (vertical segment)
        const middlePoint = {
          x: visibleStart.x,
          y: visibleStart.y - liftDistance
        };
        
        // Three-point path: vertical up, then diagonal to target
        displayPoints = [visibleStart, middlePoint, visibleEnd];
      }

      // **KEY FIX**: Shorten the path by arrowSize so the stroke doesn't overlap the arrow
      // The arrow marker will "extend" from the shortened end to the original target
      if (displayPoints.length >= 2) {
        const lastIdx = displayPoints.length - 1;
        const prevIdx = lastIdx - 1;
        const dx = displayPoints[lastIdx].x - displayPoints[prevIdx].x;
        const dy = displayPoints[lastIdx].y - displayPoints[prevIdx].y;
        const segmentLen = Math.sqrt(dx * dx + dy * dy);
        
        if (segmentLen > arrowSize) {
          // Shorten the last segment by arrowSize
          const shortenRatio = (segmentLen - arrowSize) / segmentLen;
          displayPoints[lastIdx] = {
            x: displayPoints[prevIdx].x + dx * shortenRatio,
            y: displayPoints[prevIdx].y + dy * shortenRatio
          };
        }
      }

      // Label position: geometric middle of the path
      const labelPos = this.getEdgeLabelPosition(displayPoints);

      return {
        v: e.v,
        w: e.w,
        points: displayPoints,
        markerId: `edge-marker-${e.v}-${e.w}-${index}`,
        labelPos,
        strokeColor,
        strokeWidth,
        arrowSize,
        arrowRefX,
        arrowRefY,
        arrowPoints,
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
    // getBoundingClientRect returns scaled size, so divide by zoomLevel to get original
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
      
      if (sourceNode && targetNode && edge.points && edge.points.length >= 2) {
        const targetActualH = targetNode.actualHeight || targetNode.height;
        
        // Source node: arrow starts from its TOP edge
        const newStartY = sourceNode.y - sourceNode.height / 2;
        // Target node: arrow ends at its BOTTOM edge (original target position)
        const originalEndY = (targetNode.y - targetNode.height / 2) + targetActualH;
        
        const hasMultipleChildren = targetNode.children && targetNode.children.length >= 2;
        
        if (hasMultipleChildren && edge.points.length === 3) {
          // 3-point path: [start, middle, end] - vertical + diagonal pattern
          const deltaY = Math.abs(newStartY - originalEndY);
          const liftDistance = Math.min(50, deltaY * 0.35);
          
          edge.points[0] = { x: edge.points[0].x, y: newStartY };
          edge.points[1] = { x: edge.points[0].x, y: newStartY - liftDistance };
          edge.points[2] = { x: edge.points[2].x, y: originalEndY };
        } else {
          // 2-point path: direct line
          edge.points[0] = { x: edge.points[0].x, y: newStartY };
          edge.points[edge.points.length - 1] = { x: edge.points[edge.points.length - 1].x, y: originalEndY };
        }
        
        // **KEY FIX**: Shorten the last segment by arrowSize so stroke doesn't overlap arrow
        const arrowSize = edge.arrowSize || 12;
        const lastIdx = edge.points.length - 1;
        const prevIdx = lastIdx - 1;
        const dx = edge.points[lastIdx].x - edge.points[prevIdx].x;
        const dy = edge.points[lastIdx].y - edge.points[prevIdx].y;
        const segmentLen = Math.sqrt(dx * dx + dy * dy);
        
        if (segmentLen > arrowSize) {
          const shortenRatio = (segmentLen - arrowSize) / segmentLen;
          edge.points[lastIdx] = {
            x: edge.points[prevIdx].x + dx * shortenRatio,
            y: edge.points[prevIdx].y + dy * shortenRatio
          };
        }
        
        // Update label position
        edge.labelPos = this.getEdgeLabelPosition(edge.points);
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
      // Calculate scaled dimensions
      const scaledWidth = this.graphWidth * this.zoomLevel;
      const scaledHeight = this.graphHeight * this.zoomLevel;
      // Center the graph
      this.translateX = Math.round((vw - scaledWidth) / 2);
      this.translateY = Math.round((vh - scaledHeight) / 2);
      // Ensure some padding if content is larger than viewport
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

  // Calculate schedule time percentage
  getScheduleTimePercentage(): number {
    const scheduleMs = this.analysisData?.summary?.query_peak_schedule_time_ms || 0;
    const totalMs = this.analysisData?.summary?.query_execution_wall_time_ms || 1;
    return totalMs > 0 ? Math.min((scheduleMs / totalMs) * 100, 100) : 0;
  }

  // Calculate network time percentage
  getNetworkTimePercentage(): number {
    const networkMs = this.analysisData?.summary?.query_cumulative_network_time_ms || 0;
    const totalMs = this.analysisData?.summary?.query_execution_wall_time_ms || 1;
    return totalMs > 0 ? Math.min((networkMs / totalMs) * 100, 100) : 0;
  }

  // Calculate result deliver time percentage
  getResultDeliverTimePercentage(): number {
    const deliverMs = this.analysisData?.summary?.result_deliver_time_ms || 0;
    const totalMs = this.analysisData?.summary?.query_execution_wall_time_ms || 1;
    return totalMs > 0 ? Math.min((deliverMs / totalMs) * 100, 100) : 0;
  }
  
  // Format time string with percentage
  formatTimeWithPercent(timeStr: string | undefined, percent: number): string {
    if (!timeStr) return '-';
    return `${percent.toFixed(1)}%(${timeStr})`;
  }

  // Check if IO metrics are available (for disaggregated storage)
  hasIoMetrics(): boolean {
    const s = this.analysisData?.summary;
    return !!(s?.io_seek_time || s?.local_disk_read_io_time || s?.remote_read_io_time);
  }

  // Check if IO statistics are available
  hasIoStatistics(): boolean {
    const s = this.analysisData?.summary;
    return !!(s?.total_raw_rows_read || s?.total_bytes_read || 
              s?.pages_count_memory || s?.pages_count_local_disk || s?.pages_count_remote ||
              s?.result_rows || s?.result_bytes);
  }

  // Scroll to diagnosis section
  scrollToDiagnosis(): void {
    this.isDiagnosisCollapsed = false;
    setTimeout(() => {
      const diagSection = document.querySelector('.diagnosis-section');
      if (diagSection) {
        diagSection.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    }, 100);
  }
  
  // Check if any summary metric is available
  hasAnyMetric(): boolean {
    const s = this.analysisData?.summary;
    if (!s) return false;
    return !!(s.query_execution_wall_time || s.query_cumulative_operator_time ||
              s.query_cumulative_cpu_time || s.query_cumulative_scan_time ||
              s.query_cumulative_network_time || s.query_peak_schedule_time ||
              s.result_deliver_time || s.query_sum_memory_usage || s.query_spill_bytes ||
              s.datacache_hit_rate != null);
  }
  
  // Get DataCache tooltip with detailed metrics
  getDataCacheTooltip(): string {
    const s = this.analysisData?.summary;
    if (!s) return '';
    
    const local = s.datacache_bytes_local_display || '0 B';
    const remote = s.datacache_bytes_remote_display || '0 B';
    const hitRate = s.datacache_hit_rate != null ? (s.datacache_hit_rate * 100).toFixed(1) : '0';
    
    return `存算分离架构 DataCache 命中率\n` +
           `本地缓存读取: ${local}\n` +
           `远程存储读取: ${remote}\n` +
           `命中率: ${hitRate}%\n` +
           `(命中率 >= 70% 为健康)`;
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

  // ============================================
  // Node Detail View Methods
  // ============================================

  // Get operator type from node name
  getOperatorType(node: any): string {
    if (!node?.operator_name) return 'DEFAULT';
    const name = node.operator_name.toUpperCase();
    if (name.includes('SCAN')) return 'SCAN';
    if (name.includes('JOIN')) return 'JOIN';
    if (name.includes('AGG')) return 'AGGREGATE';
    if (name.includes('EXCHANGE')) return 'EXCHANGE';
    if (name.includes('SORT') || name.includes('TOP')) return 'SORT';
    if (name.includes('PROJECT')) return 'PROJECT';
    return 'DEFAULT';
  }

  // Get execution time breakdown - dynamically detect from node's own metrics
  // If node has ScanTime -> show CPUTime + ScanTime
  // If node has NetworkTime -> show CPUTime + NetworkTime
  // Otherwise -> show CPUTime only (100%)
  getExecutionTimeBreakdown(): { items: { label: string; time: string; percent: number; color: string }[] } | null {
    if (!this.selectedNode) return null;
    
    const allMetrics = this.getAllNodeMetrics();
    const items: { label: string; time: string; percent: number; color: string }[] = [];
    
    // Get CPUTime from OperatorTotalTime
    const cpuTimeStr = allMetrics['__MAX_OF_OperatorTotalTime'] || allMetrics['OperatorTotalTime'] || '0ns';
    const cpuTimeNs = this.parseTimeToNs(cpuTimeStr);
    
    // Dynamically detect secondary time from the node's own metrics
    let secondaryTimeStr = '';
    let secondaryLabel = '';
    let secondaryTimeNs = 0;
    
    // Check what time metrics this node actually has
    if (allMetrics['ScanTime'] && this.parseTimeToNs(allMetrics['ScanTime']) > 0) {
      secondaryTimeStr = allMetrics['ScanTime'];
      secondaryLabel = 'ScanTime';
      secondaryTimeNs = this.parseTimeToNs(secondaryTimeStr);
    } else if (allMetrics['NetworkTime'] && this.parseTimeToNs(allMetrics['NetworkTime']) > 0) {
      secondaryTimeStr = allMetrics['NetworkTime'];
      secondaryLabel = 'NetworkTime';
      secondaryTimeNs = this.parseTimeToNs(secondaryTimeStr);
    }
    
    const totalTimeNs = cpuTimeNs + secondaryTimeNs || 1;
    
    // Add CPUTime
    items.push({
      label: 'CPUTime',
      time: cpuTimeStr,
      percent: secondaryTimeNs > 0 ? (cpuTimeNs / totalTimeNs) * 100 : 100,
      color: 'cpu'
    });
    
    // Add secondary time if this node has it
    if (secondaryLabel && secondaryTimeNs > 0) {
      items.push({
        label: secondaryLabel,
        time: secondaryTimeStr,
        percent: (secondaryTimeNs / totalTimeNs) * 100,
        color: 'secondary'
      });
    }
    
    return { items };
  }

  // Get all metrics from node (combining all sources)
  private getAllNodeMetrics(): { [key: string]: any } {
    if (!this.selectedNode) return {};
    
    // Combine metrics from node itself
    const result: { [key: string]: any } = {
      ...this.selectedNode.metrics,
      ...this.selectedNode.unique_metrics,
      ...this.selectedNode.aggregated_metrics
    };
    
    // Also get metrics from operators in pipeline
    const operators = this.getNodeOperatorsFromPipeline();
    for (const op of operators) {
      if (op.common_metrics) {
        Object.assign(result, op.common_metrics);
      }
      if (op.unique_metrics) {
        Object.assign(result, op.unique_metrics);
      }
    }
    
    return result;
  }

  // Get core metrics - directly from node's unique_metrics (no hardcoding!)
  // Filter out: time metrics (shown in Over Consuming), memory metrics (shown in Memory section), index metrics (shown in Indexes)
  getNodeCoreMetrics(): { key: string; value: any }[] {
    if (!this.selectedNode) return [];
    
    const allMetrics = this.getAllNodeMetrics();
    const result: { key: string; value: any }[] = [];
    
    // Keys to exclude (shown in other sections)
    const excludePatterns = [
      /Time$/i, /^__/, /Memory/i, /Peak/i, /FilterRows$/i,
      /^OperatorTotalTime$/, /^PullTotalTime$/, /^PushTotalTime$/
    ];
    
    // Priority keys to show first (common important metrics)
    const priorityKeys = [
      'RowsRead', 'BytesRead', 'CompressedBytesRead', 'ScanTime', 'IOTime',
      'Table', 'Rollup', 'Predicates',
      'BytesSent', 'BytesReceived', 'NetworkTime', 'WaitTime', 'OverallThroughput', 'PartType',
      'PullRowNum', 'PushRowNum', 'OutputRows'
    ];
    
    // First add priority keys that exist
    for (const key of priorityKeys) {
      const value = allMetrics[key];
      if (value !== undefined && value !== null && value !== '' && value !== '0' && value !== '0.000 B') {
        // Skip if matches exclude pattern
        if (!excludePatterns.some(p => p.test(key))) {
          result.push({ key, value });
        }
      }
    }
    
    // Then add other unique_metrics that weren't added
    const addedKeys = new Set(result.map(r => r.key));
    for (const [key, value] of Object.entries(allMetrics)) {
      if (addedKeys.has(key)) continue;
      if (value === undefined || value === null || value === '') continue;
      if (excludePatterns.some(p => p.test(key))) continue;
      
      // Only add non-zero values
      if (value !== '0' && value !== '0.000 B' && value !== 0) {
        result.push({ key, value });
      }
    }
    
    return result;
  }

  // Get memory metrics - dynamically find all memory-related metrics from node
  getNodeMemoryMetrics(): { key: string; value: any }[] {
    if (!this.selectedNode) return [];
    
    const allMetrics = this.getAllNodeMetrics();
    const result: { key: string; value: any }[] = [];
    
    // Find all metrics containing Memory or Peak (memory-related)
    for (const [key, value] of Object.entries(allMetrics)) {
      if (key.startsWith('__')) continue; // Skip aggregation keys
      if (!/Memory|Peak/i.test(key)) continue;
      if (value === undefined || value === null || value === '') continue;
      if (value === '0' || value === '0.000 B' || value === 0 || value === '0Bytes') continue;
      
      result.push({ key, value });
    }
    
    return result;
  }

  // Get index metrics - dynamically find all *FilterRows metrics from node
  getNodeIndexMetrics(): { key: string; value: any }[] {
    if (!this.selectedNode) return [];
    
    const allMetrics = this.getAllNodeMetrics();
    const result: { key: string; value: any }[] = [];
    
    // Index display name mapping
    const indexMapping: { [key: string]: string } = {
      'ShortKeyFilterRows': 'ShortKey',
      'ZoneMapIndexFilterRows': 'ZoneMap',
      'SegmentZoneMapFilterRows': 'ZoneMap',
      'SegmentRuntimeZoneMapFilterRows': 'RuntimeZoneMap',
      'BitmapIndexFilterRows': 'Bitmap',
      'BloomFilterFilterRows': 'Bloom'
    };
    
    // Find all FilterRows metrics
    for (const [sourceKey, displayKey] of Object.entries(indexMapping)) {
      const value = allMetrics[sourceKey];
      if (value !== undefined) {
        result.push({ key: displayKey, value });
      }
    }
    
    return result;
  }

  // Get over consuming metrics - dynamically find all time-related metrics from node
  // Format: "value[max=xxx, min=xxx]" like official UI
  getOverConsumingMetrics(): { key: string; value: string; indent?: number }[] {
    if (!this.selectedNode) return [];
    
    const allMetrics = this.getAllNodeMetrics();
    const result: { key: string; value: string; indent?: number }[] = [];
    
    // Time pattern: ends with Time or contains time units (ns/us/ms/s/m/h)
    const isTimeMetric = (key: string, value: any): boolean => {
      if (key.startsWith('__')) return false;
      if (!/Time$/i.test(key)) return false;
      if (key === 'OperatorTotalTime') return false; // Already shown in Execution time
      return true;
    };
    
    // Collect all time metrics from this node
    const timeMetrics: { key: string; value: string; timeNs: number }[] = [];
    
    for (const [key, value] of Object.entries(allMetrics)) {
      if (!isTimeMetric(key, value)) continue;
      if (value === undefined || value === null || value === '') continue;
      
      const timeNs = this.parseTimeToNs(value);
      if (timeNs <= 0) continue;
      
      // Build display value with max/min if available
      const maxVal = allMetrics[`__MAX_OF_${key}`];
      const minVal = allMetrics[`__MIN_OF_${key}`];
      let displayValue = String(value);
      if (maxVal && minVal) {
        displayValue = `${value}[max=${maxVal}, min=${minVal}]`;
      }
      
      timeMetrics.push({ key, value: displayValue, timeNs });
    }
    
    // Sort by time (descending) - show most time-consuming first
    timeMetrics.sort((a, b) => b.timeNs - a.timeNs);
    
    // Add to result
    for (const metric of timeMetrics) {
      result.push({ key: metric.key, value: metric.value, indent: 0 });
    }
    
    return result;
  }

  // Get all metrics for node detail view
  getNodeDetailMetrics(): { category: string; metrics: { key: string; value: any; description?: string }[] }[] {
    if (!this.selectedNode) return [];
    
    const categories: { category: string; metrics: { key: string; value: any; description?: string }[] }[] = [];
    
    // Unique metrics (non-aggregated)
    if (this.selectedNode.unique_metrics && Object.keys(this.selectedNode.unique_metrics).length > 0) {
      const metrics = Object.entries(this.selectedNode.unique_metrics)
        .filter(([_, v]) => v !== null && v !== undefined)
        .map(([key, value]) => ({
          key,
          value,
          description: this.metricDescriptions[key]
        }));
      if (metrics.length > 0) {
        categories.push({ category: '唯一指标', metrics });
      }
    }
    
    // Aggregated metrics (with min/max/avg)
    if (this.selectedNode.aggregated_metrics && Object.keys(this.selectedNode.aggregated_metrics).length > 0) {
      const metrics = Object.entries(this.selectedNode.aggregated_metrics)
        .filter(([_, v]) => v !== null && v !== undefined)
        .map(([key, value]) => ({
          key,
          value,
          description: this.metricDescriptions[key]
        }));
      if (metrics.length > 0) {
        categories.push({ category: '聚合指标', metrics });
      }
    }
    
    return categories;
  }

  // Get pipeline info for selected node
  getNodePipelineInfo(): any[] {
    if (!this.selectedNode) return [];
    
    // Pipeline info is typically in the node's children or specialized metrics
    const pipelineMetrics = [
      'DriverTotalTime', 'ActiveTime', 'PendingTime', 'ScheduleTime',
      'InputEmptyTime', 'OutputFullTime', 'PreconditionBlockTime',
      'LocalRfWaitingTime', 'PipelineID', 'DriverNum'
    ];
    
    const result: { key: string; value: any }[] = [];
    const allMetrics = {
      ...this.selectedNode.unique_metrics,
      ...this.selectedNode.aggregated_metrics
    };
    
    for (const key of pipelineMetrics) {
      if (allMetrics[key] !== undefined && allMetrics[key] !== null) {
        result.push({ key, value: allMetrics[key] });
      }
    }
    
    return result;
  }

  // Parse time string to nanoseconds (e.g., "84.535us" -> 84535, "1s183ms" -> 1183000000)
  private parseTimeToNs(timeStr: any): number {
    if (!timeStr) return 0;
    if (typeof timeStr === 'number') return timeStr;
    
    const str = String(timeStr).trim();
    let totalNs = 0;
    
    // Match patterns like "1h30m", "1s183ms", "84.535us", "0ns"
    const patterns = [
      { regex: /([\d.]+)h/i, multiplier: 3600000000000 },
      { regex: /([\d.]+)m(?!s)/i, multiplier: 60000000000 },
      { regex: /([\d.]+)s(?!$)/i, multiplier: 1000000000 },
      { regex: /([\d.]+)ms/i, multiplier: 1000000 },
      { regex: /([\d.]+)us/i, multiplier: 1000 },
      { regex: /([\d.]+)ns/i, multiplier: 1 },
      { regex: /^([\d.]+)s$/i, multiplier: 1000000000 }, // Just seconds
    ];
    
    for (const { regex, multiplier } of patterns) {
      const match = str.match(regex);
      if (match) {
        totalNs += parseFloat(match[1]) * multiplier;
      }
    }
    
    return totalNs;
  }

  // Calculate IO percentage for node (IO time / Total time)
  getNodeIoPercentage(): number {
    if (!this.selectedNode) return 0;
    
    // Get IOTime from various sources
    const ioTimeStr = this.selectedNode.aggregated_metrics?.IOTime || 
                      this.selectedNode.unique_metrics?.IOTime ||
                      this.selectedNode.metrics?.IOTime || 0;
    const ioTimeNs = this.parseTimeToNs(ioTimeStr);
    
    // Get total operator time
    const totalTimeNs = this.selectedNode.metrics?.operator_total_time_ns || 
                        this.parseTimeToNs(this.selectedNode.metrics?.operator_total_time) || 1;
    
    return totalTimeNs > 0 ? Math.min((ioTimeNs / totalTimeNs) * 100, 100) : 0;
  }

  // Calculate Processing percentage for node (100% - IO%)
  getNodeProcessingPercentage(): number {
    if (!this.selectedNode) return 0;
    const ioPercent = this.getNodeIoPercentage();
    return Math.max(100 - ioPercent, 0);
  }

  // Clear node selection
  clearNodeSelection(): void {
    this.selectedNode = null;
    this.selectedNodeTab = 'core';
  }

  // Select node and switch to detail view
  selectNodeForDetail(node: any): void {
    this.selectedNode = node;
    this.selectedNodeTab = 'core';
  }

  // Get Fragment info for selected node
  getNodeFragmentInfo(): any | null {
    if (!this.selectedNode?.fragment_id || !this.analysisData?.fragments) return null;
    return this.analysisData.fragments.find((f: any) => f.id === this.selectedNode.fragment_id);
  }

  // Get Fragment metrics (excluding pipelines and operators)
  getFragmentMetrics(): { key: string; value: any }[] {
    const fragment = this.getNodeFragmentInfo();
    if (!fragment) return [];
    
    const result: { key: string; value: any }[] = [];
    // Add basic fragment info
    if (fragment.backend_addresses?.length) {
      result.push({ key: 'BackendAddresses', value: fragment.backend_addresses.join(', ') });
    }
    if (fragment.instance_ids?.length) {
      result.push({ key: 'InstanceNum', value: fragment.instance_ids.length });
    }
    return result;
  }

  // Get Pipeline info for selected node from fragments
  getNodePipelineFromFragments(): any | null {
    const fragment = this.getNodeFragmentInfo();
    if (!fragment || !this.selectedNode?.pipeline_id) return null;
    return fragment.pipelines?.find((p: any) => p.id === this.selectedNode.pipeline_id);
  }

  // Get Pipeline metrics
  getPipelineMetrics(): { key: string; value: any }[] {
    const pipeline = this.getNodePipelineFromFragments();
    if (!pipeline?.metrics) return [];
    
    return Object.entries(pipeline.metrics)
      .filter(([_, v]) => v !== null && v !== undefined)
      .map(([key, value]) => ({ key, value }));
  }

  // Get operators for selected node from pipeline (for 节点详情 Tab)
  getNodeOperatorsFromPipeline(): any[] {
    const pipeline = this.getNodePipelineFromFragments();
    if (!pipeline?.operators) return [];
    
    const planNodeId = this.selectedNode?.plan_node_id?.toString();
    if (!planNodeId) return pipeline.operators;
    
    // Filter operators by plan_node_id
    return pipeline.operators.filter((op: any) => op.plan_node_id === planNodeId);
  }

  // Get all operators in the pipeline (for full view)
  getAllPipelineOperators(): any[] {
    const pipeline = this.getNodePipelineFromFragments();
    return pipeline?.operators || [];
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

  // Get diagnostics for a specific node by plan_node_id
  getNodeDiagnostics(node: any): any[] {
    // Note: plan_node_id can be 0, so use explicit null/undefined check
    if (node?.plan_node_id == null || !this.analysisData?.node_diagnostics) {
      return [];
    }
    return this.analysisData.node_diagnostics[node.plan_node_id] || [];
  }

  // Get current diagnostics count based on context (node-specific or aggregated)
  getCurrentDiagnosticsCount(): number {
    if (this.selectedNode) {
      return this.getNodeDiagnostics(this.selectedNode)?.length || 0;
    }
    return this.analysisData?.aggregated_diagnostics?.length || 0;
  }
  
  /**
   * Get total recommendations count (LLM + rule-based diagnostics)
   */
  getTotalRecommendationsCount(): number {
    const llmCount = this.hasLLMAnalysis() ? (this.getRootCauseAnalysis()?.recommendations?.length || 0) : 0;
    const ruleCount = this.analysisData?.aggregated_diagnostics?.length || 0;
    return llmCount + ruleCount;
  }
  
  /**
   * Get effective recommendations count (LLM overrides rules when available)
   */
  getEffectiveRecommendationsCount(): number {
    const llmCount = this.hasLLMAnalysis() ? (this.getRootCauseAnalysis()?.recommendations?.length || 0) : 0;
    // If LLM has recommendations, only count LLM; otherwise count rules
    if (llmCount > 0) {
      return llmCount;
    }
    return this.analysisData?.aggregated_diagnostics?.length || 0;
  }
  
  /**
   * Get merged root cause analysis data
   * Priority: LLM analysis (if available and completed) > Rule-based analysis
   * LLM analysis provides richer natural language descriptions
   */
  getRootCauseAnalysis(): any {
    const llmAnalysis = this.analysisData?.llm_analysis;
    const ruleAnalysis = this.analysisData?.root_cause_analysis;
    
    // If LLM analysis is available and completed, use it
    if (llmAnalysis?.available && llmAnalysis?.status === 'completed') {
      return {
        root_causes: llmAnalysis.root_causes || [],
        causal_chains: llmAnalysis.causal_chains || [],
        summary: llmAnalysis.summary || '',
        recommendations: llmAnalysis.merged_recommendations || [],
        hidden_issues: llmAnalysis.hidden_issues || [],
        source: 'llm'
      };
    }
    
    // Fallback to rule-based analysis
    if (ruleAnalysis) {
      return {
        ...ruleAnalysis,
        source: 'rule'
      };
    }
    
    return null;
  }
  
  /**
   * Check if LLM analysis is available
   */
  hasLLMAnalysis(): boolean {
    const llmAnalysis = this.analysisData?.llm_analysis;
    return llmAnalysis?.available && llmAnalysis?.status === 'completed';
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

  // Zoom controls - use 0.25 steps for crisper rendering (avoid fractional pixels)
  zoomIn(): void {
    this.zoomLevel = Math.min(Math.round((this.zoomLevel + 0.25) * 4) / 4, 3);
  }

  zoomOut(): void {
    this.zoomLevel = Math.max(Math.round((this.zoomLevel - 0.25) * 4) / 4, 0.25);
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

  // Toggle graph direction (BT <-> LR)
  toggleGraphDirection(): void {
    this.graphDirection = this.graphDirection === 'BT' ? 'LR' : 'BT';
    // Rebuild graph with new direction
    if (this.analysisData?.execution_tree) {
      this.buildGraph(this.analysisData.execution_tree);
    }
  }

  // Toggle data flow animation
  toggleFlowAnimation(): void {
    this.isFlowAnimationEnabled = !this.isFlowAnimationEnabled;
  }

  // Export DAG as PNG image
  exportDagAsPng(): void {
    const graphContent = document.querySelector('.graph-content') as HTMLElement;
    if (!graphContent) {
      this.toastrService.warning('无法找到图表内容', '导出失败');
      return;
    }

    // Use html2canvas to capture the graph
    import('html2canvas').then(html2canvasModule => {
      const html2canvas = html2canvasModule.default;
      html2canvas(graphContent, {
        backgroundColor: '#ffffff',
        scale: 2, // Higher resolution
        logging: false,
        useCORS: true,
      }).then(canvas => {
        // Create download link
        const link = document.createElement('a');
        link.download = `profile-dag-${this.currentQueryId || 'export'}.png`;
        link.href = canvas.toDataURL('image/png');
        link.click();
        this.toastrService.success('图表已导出', '导出成功');
      }).catch(err => {
        console.error('Export failed:', err);
        this.toastrService.danger('导出图片失败', '错误');
      });
    }).catch(err => {
      console.error('Failed to load html2canvas:', err);
      this.toastrService.danger('加载导出模块失败', '错误');
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

  private normalizedOperatorName(node: any): string {
    return (node?.operator_name || '').toUpperCase();
  }

  isScanNode(node: any): boolean {
    return this.normalizedOperatorName(node).includes('SCAN');
  }

  isJoinNode(node: any): boolean {
    return this.normalizedOperatorName(node).includes('JOIN');
  }

  isProjectNode(node: any): boolean {
    return this.normalizedOperatorName(node).includes('PROJECT');
  }

  isExchangeNode(node: any): boolean {
    return this.normalizedOperatorName(node).includes('EXCHANGE');
  }

  isAggregationNode(node: any): boolean {
    const name = this.normalizedOperatorName(node);
    return name.includes('AGGREGATION') || name.includes('AGG');
  }

  // Get node header class based on operator type (Figure 1 style)
  getNodeHeaderClass(node: any): string {
    if (this.getNodeRank(node) === 1) {
      return 'header-red';
    }
    if (this.isScanNode(node) || this.isJoinNode(node)) {
      return 'header-orange';
    }
    return 'header-gray';
  }

  // Get progress bar color based on node rank and type
  // Top 1: Red (most time consuming)
  // Top 2/3 or SCAN/JOIN: Orange
  // Others: Gray
  getProgressColor(node: any): string {
    const rank = this.getNodeRank(node);
    if (rank === 1) return '#cf1322';  // Red for Top 1
    if (rank > 0 || this.isScanNode(node) || this.isJoinNode(node)) return '#fa8c16';  // Orange
    return '#d9d9d9';  // Gray
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

  // Copy text to clipboard (for parameter commands)
  copyToClipboard(text: string): void {
    if (!text) return;
    
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(text)
        .then(() => {
          this.toastrService.success('已复制到剪贴板', '复制成功');
        })
        .catch(err => {
          console.error('Failed to copy:', err);
          this.toastrService.warning('复制失败', '提示');
        });
    } else {
      // Fallback
      const textArea = document.createElement('textarea');
      textArea.value = text;
      textArea.style.position = 'fixed';
      textArea.style.left = '-999999px';
      document.body.appendChild(textArea);
      textArea.select();
      try {
        document.execCommand('copy');
        this.toastrService.success('已复制到剪贴板', '复制成功');
      } catch (err) {
        this.toastrService.warning('复制失败', '提示');
      } finally {
        document.body.removeChild(textArea);
      }
    }
  }
}

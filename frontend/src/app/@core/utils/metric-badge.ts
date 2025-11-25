export interface MetricThresholds {
  warn: number;
  danger: number;
  reverse?: boolean;
}

export interface DynamicThresholdOptions {
  // 数据采样策略
  sampleSize?: number;           // 采样数量，默认1000
  percentileMethod?: 'p75' | 'p80' | 'p85' | 'p90' | 'p95'; // 百分位方法，默认p85
  
  // 阈值计算规则
  warnMultiplier?: number;      // 警告阈值倍数，基于P75计算，默认2.0
  dangerMultiplier?: number;     // 危险阈值倍数，基于P90计算，默认3.0
  
  // 最小阈值保护
  minWarnThreshold?: number;     // 最小警告阈值（毫秒），默认30000 (30秒)
  minDangerThreshold?: number;   // 最小危险阈值（毫秒），默认120000 (2分钟)
  
  // 最大阈值保护
  maxWarnThreshold?: number;     // 最大警告阈值（毫秒），默认300000 (5分钟)
  maxDangerThreshold?: number;   // 最大危险阈值（毫秒），默认600000 (10分钟)
}

export type MetricStatus = 'good' | 'warn' | 'alert' | 'neutral';

export interface MetricBadgeOptions {
  labelFormatter?: (value: string | number) => string;
  fallbackLabel?: string;
}

export function renderMetricBadge(
  rawValue: string | number | null | undefined,
  thresholds: MetricThresholds,
  options: MetricBadgeOptions = {},
): string {
  const { labelFormatter, fallbackLabel = '--' } = options;
  const numericValue = extractNumericValue(rawValue);
  const label = formatLabel(rawValue, fallbackLabel, labelFormatter);

  if (Number.isNaN(numericValue)) {
    return `<span class="metric-badge metric-badge--neutral">${label}</span>`;
  }

  const status = resolveMetricStatus(numericValue, thresholds);
  return `<span class="metric-badge metric-badge--${status}">${label}</span>`;
}

export function extractNumericValue(value: string | number | null | undefined): number {
  if (typeof value === 'number') {
    return value;
  }

  if (value === null || value === undefined) {
    return Number.NaN;
  }

  const normalized = value
    .toString()
    .replace(/[,+]/g, '')
    .replace(/[^0-9.-]/g, '');

  if (!normalized) {
    return Number.NaN;
  }

  return parseFloat(normalized);
}

/**
 * Parse StarRocks duration string (e.g., "1m41s", "35s370ms", "1ms41s") to milliseconds
 * @param durationStr - Duration string from StarRocks
 * @returns Duration in milliseconds
 */
export function parseStarRocksDuration(durationStr: string | number | null | undefined): number {
  if (typeof durationStr === 'number') {
    return durationStr;
  }

  if (!durationStr) {
    return Number.NaN;
  }

  const str = durationStr.toString().trim();
  let totalMs = 0;
  
  // Match patterns like: 1m41s, 35s370ms, 1ms41s, 1h2m3s4ms
  // Order matters: match longer units first to avoid conflicts (ms before m, s before sec)
  const regex = /(\d+)(h|hr|hour|ms|millisec|m|min|s|sec)/gi;
  let match;
  
  while ((match = regex.exec(str)) !== null) {
    const value = parseInt(match[1], 10);
    const unit = match[2].toLowerCase();
    
    switch (unit) {
      case 'h':
      case 'hr':
      case 'hour':
        totalMs += value * 60 * 60 * 1000;
        break;
      case 'm':
      case 'min':
        totalMs += value * 60 * 1000;
        break;
      case 's':
      case 'sec':
        totalMs += value * 1000;
        break;
      case 'ms':
      case 'millisec':
        totalMs += value;
        break;
    }
  }
  
  // If no units found, try to parse as plain number (assume milliseconds)
  if (totalMs === 0 && /^\d+$/.test(str)) {
    totalMs = parseInt(str, 10);
  }
  
  return totalMs || Number.NaN;
}

/**
 * Calculate dynamic thresholds based on data distribution
 * @param values - Array of duration values in milliseconds
 * @param options - Configuration for dynamic threshold calculation
 * @returns Calculated thresholds
 */
export function calculateDynamicThresholds(
  values: number[],
  options: DynamicThresholdOptions = {}
): MetricThresholds {
  const {
    sampleSize = 1000,
    percentileMethod = 'p85',
    warnMultiplier = 2.0,
    dangerMultiplier = 3.0,
    minWarnThreshold = 30000,    // 30 seconds
    minDangerThreshold = 120000, // 2 minutes
    maxWarnThreshold = 300000,   // 5 minutes
    maxDangerThreshold = 600000,  // 10 minutes
  } = options;

  // Filter valid values and sample if needed
  const validValues = values
    .filter(v => !isNaN(v) && v > 0)
    .sort((a, b) => a - b);
  
  if (validValues.length === 0) {
    // Fallback to default thresholds
    return { warn: minWarnThreshold, danger: minDangerThreshold };
  }

  // Sample data if too large
  const sampledValues = validValues.length > sampleSize 
    ? sampleData(validValues, sampleSize)
    : validValues;

  // Calculate percentiles
  const percentiles = calculatePercentiles(sampledValues);
  
  // Get base values for calculation
  const baseWarnValue = percentiles[percentileMethod];
  const baseDangerValue = percentiles.p95;

  // Calculate thresholds with multipliers
  let warnThreshold = Math.round(baseWarnValue * warnMultiplier);
  let dangerThreshold = Math.round(baseDangerValue * dangerMultiplier);

  // Apply min/max constraints
  warnThreshold = Math.max(minWarnThreshold, Math.min(warnThreshold, maxWarnThreshold));
  dangerThreshold = Math.max(minDangerThreshold, Math.min(dangerThreshold, maxDangerThreshold));

  // Ensure danger threshold is greater than warn threshold
  if (dangerThreshold <= warnThreshold) {
    dangerThreshold = Math.round(warnThreshold * 1.5);
  }

  return { warn: warnThreshold, danger: dangerThreshold };
}

/**
 * Sample data evenly from the array
 */
function sampleData(values: number[], sampleSize: number): number[] {
  const step = Math.floor(values.length / sampleSize);
  const sampled: number[] = [];
  
  for (let i = 0; i < values.length && sampled.length < sampleSize; i += step) {
    sampled.push(values[i]);
  }
  
  return sampled;
}

/**
 * Calculate common percentiles from sorted data
 */
function calculatePercentiles(sortedValues: number[]): Record<string, number> {
  const n = sortedValues.length;
  
  const getPercentile = (p: number): number => {
    const index = Math.ceil((p / 100) * n) - 1;
    return sortedValues[Math.max(0, Math.min(index, n - 1))];
  };

  return {
    p50: getPercentile(50),
    p75: getPercentile(75),
    p80: getPercentile(80),
    p85: getPercentile(85),
    p90: getPercentile(90),
    p95: getPercentile(95),
    p99: getPercentile(99),
  };
}

export function resolveMetricStatus(value: number, thresholds: MetricThresholds): MetricStatus {
  const { warn, danger, reverse } = thresholds;

  if (reverse) {
    if (value <= danger) {
      return 'alert';
    }
    if (value <= warn) {
      return 'warn';
    }
    return 'good';
  }

  if (value >= danger) {
    return 'alert';
  }
  if (value >= warn) {
    return 'warn';
  }
  return 'good';
}

function formatLabel(
  rawValue: string | number | null | undefined,
  fallback: string,
  formatter?: (value: string | number) => string,
): string {
  if (formatter && rawValue !== null && rawValue !== undefined && rawValue !== '') {
    return formatter(rawValue);
  }

  if (rawValue === null || rawValue === undefined || rawValue === '') {
    return fallback;
  }

  return rawValue.toString().trim();
}

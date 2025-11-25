export interface MetricThresholds {
  warn: number;
  danger: number;
  reverse?: boolean;
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

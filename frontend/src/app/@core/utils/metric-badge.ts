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

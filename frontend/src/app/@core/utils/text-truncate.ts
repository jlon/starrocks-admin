/**
 * Utility functions for text truncation with tooltip support
 */

/**
 * Render long text with truncation and tooltip
 * @param value - The text value to render
 * @param maxLength - Maximum length before truncation (default: 50)
 * @returns HTML string with truncated text and title attribute for tooltip
 */
export function renderLongText(value: any, maxLength: number = 50): string {
  if (value === null || value === undefined) {
    return '-';
  }
  const text = String(value);
  if (text.length <= maxLength) {
    // Even if not truncated, add title for consistency
    const escapedText = escapeHtmlForAttribute(text);
    return `<span title="${escapedText}">${escapeHtmlForContent(text)}</span>`;
  }
  const truncated = text.substring(0, maxLength) + '...';
  // Return HTML with title attribute for tooltip
  const escapedText = escapeHtmlForAttribute(text);
  const escapedTruncated = escapeHtmlForContent(truncated);
  return `<span title="${escapedText}" style="cursor: help;">${escapedTruncated}</span>`;
}

/**
 * Escape HTML for use in HTML attributes (like title)
 */
function escapeHtmlForAttribute(text: string): string {
  if (!text) return '';
  const div = document.createElement('div');
  div.textContent = text;
  // Replace quotes and other special characters for attributes
  return div.innerHTML
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

/**
 * Escape HTML for use in HTML content
 */
function escapeHtmlForContent(text: string): string {
  if (!text) return '';
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}


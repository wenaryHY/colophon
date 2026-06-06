/** HTML 转义，防止 XSS */
export function esc(s: string | null | undefined): string {
  if (!s) return '';
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

/**
 * Best-effort client-side preview of the slug the backend will generate from a
 * title when no custom slug is provided. It mirrors the backend's ASCII rules
 * (lowercase, fold Latin diacritics, collapse runs of non-alphanumeric chars to
 * a single '-', trim leading/trailing '-'). The backend additionally romanizes
 * CJK characters to pinyin via `deunicode`, which cannot be reproduced here
 * without a large transliteration table, so this preview is only a hint — the
 * backend remains the source of truth for the persisted slug.
 */
export function generateSlugPreviewFromTitle(title: string): string {
  return title
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

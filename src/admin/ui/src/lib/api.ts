import type { PaginatedResponse } from '../types';

export const API = `${window.location.protocol}//${window.location.host}`;

/** API version prefix — all API calls use v1 */
export const API_PREFIX = '/api/v1';

export class ApiClientError extends Error {
  status: number;

  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiClientError';
    this.status = status;
  }
}

// ── 内存中的 access_token ──

let accessToken: string | null = null;

export function setAccessToken(token: string) {
  accessToken = token;
}

export function clearAccessToken() {
  accessToken = null;
}

// ── 401 自动刷新 ──

let refreshPromise: Promise<string | null> | null = null;

async function refreshAccessToken(): Promise<string | null> {
  try {
    const response = await fetch(`${API_PREFIX}/auth/refresh`, {
      method: 'POST',
      credentials: 'include',
    });
    if (!response.ok) return null;
    const json = (await response.json()) as ApiResponse<{ access_token: string }>;
    return json.data?.access_token ?? null;
  } catch {
    return null;
  }
}

async function tryRefreshToken(): Promise<boolean> {
  if (!refreshPromise) {
    refreshPromise = refreshAccessToken().finally(() => {
      refreshPromise = null;
    });
  }
  const newToken = await refreshPromise;
  if (newToken) {
    accessToken = newToken;
    return true;
  }
  clearAccessToken();
  return false;
}

// ── 核心请求函数 ──

function buildUrl(path: string): string {
  if (path.startsWith('http')) return path;
  if (path.startsWith(API_PREFIX)) return path;
  return `${API_PREFIX}${path.startsWith('/') ? '' : '/'}${path}`;
}

function buildHeaders(opts: RequestInit): Headers {
  const headers = new Headers(opts.headers as HeadersInit | undefined);
  if (accessToken) {
    headers.set('Authorization', `Bearer ${accessToken}`);
  }
  if (opts.body && !(opts.body instanceof FormData) && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }
  return headers;
}

export async function api<T = unknown>(path: string, opts: RequestInit = {}): Promise<T> {
  const url = buildUrl(path);

  let response = await fetch(url, {
    ...opts,
    credentials: 'include',
    headers: buildHeaders(opts),
  });

  // 401 自动刷新（排除 refresh 自身避免死循环）
  if (response.status === 401 && !path.includes('/auth/refresh')) {
    const refreshed = await tryRefreshToken();
    if (refreshed) {
      response = await fetch(url, {
        ...opts,
        credentials: 'include',
        headers: buildHeaders(opts),
      });
    }
  }

  if (!response.ok) {
    throw new ApiClientError(response.status, `Request failed: ${response.status}`);
  }

  return response.json() as T;
}

interface ApiResponse<T = unknown> {
  code: number;
  message: string;
  data?: T;
  request_id: string;
}

export async function apiData<T = unknown>(path: string, opts: RequestInit = {}): Promise<T> {
  const response = await api<ApiResponse<T>>(path, opts);
  return response.data as T;
}

export function paginationPages<T>(payload: PaginatedResponse<T>): number {
  return Math.max(1, Math.ceil(payload.pagination.total / payload.pagination.page_size));
}

// ── TanStack Query 全局引用 ──

import { QueryClient } from '@tanstack/react-query';

let _queryClient: QueryClient | null = null;

export function setQueryClient(client: QueryClient) {
  _queryClient = client;
}

export function getQueryClient(): QueryClient {
  if (!_queryClient) throw new Error('QueryClient not initialized');
  return _queryClient;
}

// ── MediaCategory API ──

export async function listMediaCategories() {
  return apiData<import('../types').MediaCategory[]>(`${API_PREFIX}/admin/media/categories`);
}

export async function createMediaCategory(data: import('../types').CreateMediaCategoryRequest) {
  return apiData<import('../types').MediaCategory>(`${API_PREFIX}/admin/media/categories`, {
    method: 'POST',
    body: JSON.stringify(data),
  });
}

export async function updateMediaCategory(id: string, data: import('../types').UpdateMediaCategoryRequest) {
  return apiData<import('../types').MediaCategory>(`${API_PREFIX}/admin/media/categories/${id}`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

export async function deleteMediaCategory(id: string) {
  return apiData(`${API_PREFIX}/admin/media/categories/${id}`, {
    method: 'DELETE',
  });
}

// ── Theme API ──

export async function getThemeDetail(slug: string) {
  return apiData<import('../types').ThemeDetailResponse>(`${API_PREFIX}/admin/themes/${slug}/detail`);
}

export async function saveThemeConfig(slug: string, config: Record<string, unknown>) {
  return apiData(`${API_PREFIX}/admin/themes/${slug}/config`, {
    method: 'PATCH',
    body: JSON.stringify({ config }),
  });
}

export async function activateTheme(slug: string) {
  return apiData(`${API_PREFIX}/admin/themes/${slug}/activate`, {
    method: 'POST',
  });
}

export async function uploadTheme(file: File) {
  const formData = new FormData();
  formData.append('file', file);
  return apiData<import('../types').ThemeUploadResponse>(`${API_PREFIX}/admin/themes/upload`, {
    method: 'POST',
    body: formData,
  });
}

// ── Backup API ──

export async function createBackup(provider: string = 'local') {
  return apiData(`${API_PREFIX}/admin/backup`, {
    method: 'POST',
    body: JSON.stringify({ provider }),
  });
}

export async function listBackups() {
  return apiData<import('../types').BackupListResponse[]>(`${API_PREFIX}/admin/backup/list`);
}

export async function restoreBackup(backupId: string) {
  return apiData(`${API_PREFIX}/admin/backup/restore`, {
    method: 'POST',
    body: JSON.stringify({ backup_id: backupId }),
  });
}

export async function getBackupSchedule() {
  return apiData<import('../types').BackupScheduleResponse>(`${API_PREFIX}/admin/backup/schedule`);
}

export async function updateBackupSchedule(data: import('../types').BackupScheduleRequest) {
  return apiData(`${API_PREFIX}/admin/backup/schedule`, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
}

export async function deleteBackup(id: string) {
  return apiData(`${API_PREFIX}/admin/backup/${id}`, {
    method: 'DELETE',
  });
}

export async function mergeRestoreBackup(id: string) {
  return apiData<import('../types').RestoreProgressResponse[]>(`${API_PREFIX}/admin/backups/${id}/merge-restore`, {
    method: 'POST',
  });
}

// ── 回收站 API ──

export async function listTrash(type?: string) {
  const params = type ? `?type=${type}` : '';
  return apiData<import('../types').TrashItem[]>(`${API_PREFIX}/admin/trash${params}`);
}

export async function restoreTrashItem(itemType: string, id: string) {
  return apiData(`${API_PREFIX}/admin/trash/${itemType}/${id}/restore`, {
    method: 'POST',
  });
}

export async function purgeTrashItem(itemType: string, id: string) {
  return apiData(`${API_PREFIX}/admin/trash/${itemType}/${id}`, {
    method: 'DELETE',
  });
}

export async function purgeExpiredTrash() {
  return apiData(`${API_PREFIX}/admin/trash/purge-expired`, {
    method: 'POST',
  });
}

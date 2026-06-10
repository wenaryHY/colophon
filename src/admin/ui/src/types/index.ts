export interface ApiResponse<T = unknown> {
  code: number;
  message: string;
  data: T | null;
  request_id: string;
}

export interface PaginationMeta {
  page: number;
  page_size: number;
  total: number;
}

export interface PaginatedResponse<T> {
  items: T[];
  pagination: PaginationMeta;
}

export interface Category {
  id: string;
  name: string;
  slug: string;
  description: string | null;
  parent_id: string | null;
  sort_order: number;
  created_at: string;
}

export interface Tag {
  id: string;
  name: string;
  slug: string;
  created_at: string;
}

export interface AdminPost {
  id: string;
  author_id: string;
  title: string;
  slug: string;
  excerpt: string | null;
  content_md: string;
  content_html: string;
  cover_media_id: string | null;
  status: 'draft' | 'published' | 'trashed';
  visibility: 'public' | 'private';
  category_id: string | null;
  allow_comment: boolean;
  pinned: boolean;
  content_type: 'post' | 'page';
  custom_html_path: string | null;
  page_render_mode: 'editor' | 'custom_html';
  published_at: string | null;
  created_at: string;
  updated_at: string;
  tags: Tag[];
}

export interface PublicPostSummary {
  id: string;
  title: string;
  slug: string;
  excerpt: string | null;
  content_type: 'post' | 'page';
  published_at: string | null;
  created_at: string;
  updated_at: string;
  author_display_name: string;
  category_name: string | null;
}

export interface Comment {
  id: string;
  post_id: string;
  user_id: string;
  username: string;
  display_name: string;
  content: string;
  status: 'pending' | 'approved' | 'rejected' | 'deleted';
  parent_id: string | null;
  created_at: string;
  updated_at: string;
  post_title?: string;
  post_slug?: string;
  post_content_type?: 'post' | 'page';
}

export interface Setting {
  key: string;
  value: string;
}

export type SetupStage = 'not_started' | 'admin_created' | 'configured' | 'completed';

export interface SetupStatusResponse {
  installed: boolean;
  stage: SetupStage;
  site_title: string;
  site_description: string;
  site_url: string;
  admin_url: string;
  allow_register: boolean;
}

export interface SetupInitializeResponse {
  token: string;
  redirect_to: string;
}

export interface MediaCategory {
  id: string;
  name: string;
  slug: string;
  description?: string;
  icon?: string;
  color?: string;
  sort_order: number;
  created_at: string;
}

export interface CreateMediaCategoryRequest {
  name: string;
  slug?: string;
  description?: string;
  icon?: string;
  color?: string;
  sort_order?: number;
}

export interface UpdateMediaCategoryRequest {
  name?: string;
  slug?: string;
  description?: string;
  icon?: string;
  color?: string;
  sort_order?: number;
}

export interface MediaItem {
  id: string;
  uploader_id: string;
  kind: 'image' | 'audio';
  mime_type: string;
  original_name: string;
  stored_name: string;
  storage_path: string;
  public_url: string;
  size_bytes: number;
  width: number | null;
  height: number | null;
  duration_seconds: number | null;
  alt_text: string | null;
  category: string | null;
  created_at: string;
}

export interface SelectOption {
  label: string;
  value: string;
}

export type ThemeConfigField =
  | { type: 'text'; label: string; default?: string | null }
  | { type: 'color'; label: string; default?: string | null }
  | { type: 'number'; label: string; default?: number | null }
  | { type: 'select'; label: string; default?: string | null; options: SelectOption[] };

export interface ThemeManifest {
  name: string;
  slug: string;
  version: string;
  author: string;
  description: string;
  min_colophon_version?: string;
  preview_image?: string | null;
  config?: Record<string, ThemeConfigField>;
}

export interface ThemeSummary {
  manifest: ThemeManifest;
  active: boolean;
}

export interface ThemeDetailResponse {
  manifest: ThemeManifest;
  config: Record<string, unknown>;
  schema: Record<string, ThemeConfigField>;
}

export interface SaveThemeConfigRequest {
  config: Record<string, unknown>;
}

export interface ThemeUploadResponse {
  slug: string;
  name: string;
  version: string;
  message: string;
}

export interface CurrentUser {
  id: string;
  username: string;
  email: string;
  display_name: string;
  avatar_media_id: string | null;
  bio: string | null;
  role: string;
  status: string;
  theme_preference: 'system' | 'light' | 'dark';
  language: 'zh' | 'en';
  created_at: string;
  updated_at: string;
}

export interface BackupListResponse {
  id: string;
  created_at: string;
  size: number;
  provider: string;
  status: string;
  error_message?: string | null;
}

export interface BackupScheduleResponse {
  id: string;
  enabled: boolean;
  frequency: string;
  hour: number;
  minute: number;
  provider: string;
  last_run_at?: string | null;
  next_run_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface BackupScheduleRequest {
  enabled: boolean;
  frequency: string;
  hour: number;
  minute: number;
  provider: string;
}

export interface RestoreProgressResponse {
  step: string;
  status: string;
  message: string;
}

export interface TrashItem {
  id: string;
  item_type: string;
  name: string;
  subtitle: string | null;
  deleted_at: string;
  expires_in_days: number;
}

// ── Webhook ──

export interface Webhook {
  id: string;
  name: string;
  url: string;
  events: string;
  secret?: string | null;
  enabled: number; // sqlite boolean: 0/1
  retry_count: number;
  max_retries: number;
  last_triggered_at?: string | null;
  last_error?: string | null;
  created_at: string;
  updated_at: string;
}

export interface WebhookDelivery {
  id: string;
  webhook_id: string;
  event: string;
  request_url: string;
  request_body?: string | null;
  response_status?: number | null;
  response_body?: string | null;
  duration_ms?: number | null;
  success: number; // 0/1
  created_at: string;
}

export interface CreateWebhookRequest {
  name: string;
  url: string;
  events?: string;
  secret?: string;
  enabled?: boolean;
  max_retries?: number;
}

export interface UpdateWebhookRequest {
  name?: string;
  url?: string;
  events?: string;
  secret?: string;
  enabled?: boolean;
  max_retries?: number;
}

export interface ApiKeyListItem {
  id: string;
  name: string;
  key_prefix: string;
  permissions: string;
  last_used_at: string | null;
  expires_at: string | null;
  created_at: string;
}

export interface CreateApiKeyResponse {
  id: string;
  name: string;
  key_prefix: string;
  api_key: string;
  permissions: string;
  expires_at: string | null;
  created_at: string;
}

/**
 * 媒体上传共享函数 — MediaPicker 和 Upload 页面共用
 */
import { apiData, API_PREFIX } from './api';
import type { MediaItem } from '../types';

export async function uploadMedia(file: File): Promise<MediaItem> {
  const formData = new FormData();
  formData.append('file', file);
  return apiData<MediaItem>(`${API_PREFIX}/admin/media`, {
    method: 'POST',
    body: formData,
  });
}

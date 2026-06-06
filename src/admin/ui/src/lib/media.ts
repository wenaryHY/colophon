/**
 * 媒体上传共享函数 — MediaPicker 和 Upload 页面共用
 */
import { API_PREFIX, getAccessToken } from './api';
import type { MediaItem } from '../types';

export interface UploadProgress {
  loaded: number;
  total: number;
  percentage: number;
}

export async function uploadMedia(
  file: File,
  onProgress?: (progress: UploadProgress) => void
): Promise<MediaItem> {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    const formData = new FormData();
    formData.append('file', file);

    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable && onProgress) {
        onProgress({
          loaded: e.loaded,
          total: e.total,
          percentage: Math.round((e.loaded / e.total) * 100),
        });
      }
    });

    xhr.addEventListener('load', () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          const json = JSON.parse(xhr.responseText);
          if (json.code === 0) {
            resolve(json.data as MediaItem);
          } else {
            reject(new Error(json.message || 'Upload failed'));
          }
        } catch {
          reject(new Error('Invalid response'));
        }
      } else {
        reject(new Error(`Upload failed: ${xhr.status}`));
      }
    });

    xhr.addEventListener('error', () => reject(new Error('Network error')));
    xhr.addEventListener('abort', () => reject(new Error('Upload cancelled')));

    xhr.open('POST', `${API_PREFIX}/admin/media`);
    xhr.withCredentials = true;

    // 如果内存中有 accessToken，用 Bearer header（快路径），否则靠 cookie 兜底
    const token = getAccessToken();
    if (token) {
      xhr.setRequestHeader('Authorization', `Bearer ${token}`);
    }

    xhr.send(formData);
  });
}

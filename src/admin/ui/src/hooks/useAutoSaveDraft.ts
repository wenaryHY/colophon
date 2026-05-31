import { useEffect, useCallback } from 'react';

export interface DraftData {
  title: string;
  content: string;
  contentHtml: string;
  excerpt: string;
  categoryId: string;
  tagIds: string[];
  savedAt: number;
}

const DRAFT_PREFIX = 'inkforge_draft_';

export function useAutoSaveDraft(
  postId: string | undefined,
  formData: { title: string; content: string; contentHtml: string; excerpt: string; categoryId: string; tagIds: string[] },
  enabled: boolean
) {
  const key = postId ? `${DRAFT_PREFIX}${postId}` : `${DRAFT_PREFIX}new`;

  // 自动保存：启用且内容有变化时，2 秒防抖后写入 localStorage
  useEffect(() => {
    if (!enabled) return;
    const timer = setTimeout(() => {
      try {
        localStorage.setItem(key, JSON.stringify({ ...formData, savedAt: Date.now() }));
      } catch { /* 存储满或隐私模式下忽略 */ }
    }, 2000);
    return () => clearTimeout(timer);
  }, [formData, key, enabled]);

  const restore = useCallback((): DraftData | null => {
    try {
      const raw = localStorage.getItem(key);
      return raw ? JSON.parse(raw) : null;
    } catch {
      return null;
    }
  }, [key]);

  const clear = useCallback(() => {
    localStorage.removeItem(key);
  }, [key]);

  return { restore, clear };
}

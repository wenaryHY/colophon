import { useEffect, useCallback, useState, useRef } from 'react';

export interface DraftData {
  title: string;
  content: string;
  contentHtml: string;
  excerpt: string;
  categoryId: string;
  tagIds: string[];
  savedAt: number;
}

const DRAFT_PREFIX = 'colophon_draft_';

/** 草稿最大有效时长（ms）：超过 24 小时自动失效 */
const MAX_DRAFT_AGE_MS = 24 * 60 * 60 * 1000;

export function useAutoSaveDraft(
  postId: string | undefined,
  formData: { title: string; content: string; contentHtml: string; excerpt: string; categoryId: string; tagIds: string[] },
  enabled: boolean
) {
  const key = postId ? `${DRAFT_PREFIX}${postId}` : `${DRAFT_PREFIX}new`;
  const [lastSavedAt, setLastSavedAt] = useState<number | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  // 记录上次已保存的内容指纹，避免重复保存（formData 每次渲染都是新对象引用）
  const lastSavedContentRef = useRef<string>('');

  // 自动保存：启用且内容有变化时，2 秒防抖后写入 localStorage
  useEffect(() => {
    if (!enabled) return;

    // 序列化当前内容用于比较
    const currentContent = JSON.stringify(formData);

    // 内容未变化，跳过保存
    if (currentContent === lastSavedContentRef.current) {
      setIsSaving(false);
      return;
    }

    setIsSaving(true);
    const timer = setTimeout(() => {
      try {
        localStorage.setItem(key, JSON.stringify({ ...formData, savedAt: Date.now() }));
        lastSavedContentRef.current = JSON.stringify(formData);
        setLastSavedAt(Date.now());
      } catch { /* 存储满或隐私模式下忽略 */ }
      setIsSaving(false);
    }, 2000);
    return () => clearTimeout(timer);
  }, [formData, key, enabled]);

  const restore = useCallback((): DraftData | null => {
    try {
      const raw = localStorage.getItem(key);
      if (!raw) return null;
      const draft: DraftData = JSON.parse(raw);
      // 超过最大有效时长的草稿自动失效
      if (Date.now() - draft.savedAt > MAX_DRAFT_AGE_MS) {
        localStorage.removeItem(key);
        return null;
      }
      return draft;
    } catch {
      return null;
    }
  }, [key]);

  const clear = useCallback(() => {
    localStorage.removeItem(key);
  }, [key]);

  return { restore, clear, lastSavedAt, isSaving };
}

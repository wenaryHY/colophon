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

const DRAFT_PREFIX = 'colophon-autosave-post-';

/** 草稿最大有效时长（ms）：超过 7 天自动失效（sessionStorage 本身标签关闭即清，此值作为额外保险） */
const MAX_DRAFT_AGE_MS = 7 * 24 * 60 * 60 * 1000;

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

  // 自动保存：启用且内容有变化时，2 秒防抖后写入 sessionStorage
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
        sessionStorage.setItem(key, JSON.stringify({ ...formData, savedAt: Date.now() }));
        lastSavedContentRef.current = JSON.stringify(formData);
        setLastSavedAt(Date.now());
      } catch { /* 存储满或隐私模式下忽略 */ }
      setIsSaving(false);
    }, 2000);
    return () => clearTimeout(timer);
  }, [formData, key, enabled]);

  const restore = useCallback((): DraftData | null => {
    try {
      const raw = sessionStorage.getItem(key);
      if (!raw) return null;
      const draft: DraftData = JSON.parse(raw);
      // 超过最大有效时长的草稿自动失效
      if (Date.now() - draft.savedAt > MAX_DRAFT_AGE_MS) {
        // 迭代清理所有过期草稿（倒序遍历，防止并发修改导致索引错位）
        for (let i = sessionStorage.length - 1; i >= 0; i--) {
          const k = sessionStorage.key(i);
          if (k && k.startsWith(DRAFT_PREFIX)) {
            const rawDraft = sessionStorage.getItem(k);
            if (rawDraft) {
              try {
                const d: DraftData = JSON.parse(rawDraft);
                if (Date.now() - d.savedAt > MAX_DRAFT_AGE_MS) {
                  sessionStorage.removeItem(k);
                }
              } catch { /* 格式损坏的条目跳过 */ }
            }
          }
        }
        return null;
      }
      return draft;
    } catch {
      return null;
    }
  }, [key]);

  const clear = useCallback(() => {
    sessionStorage.removeItem(key);
  }, [key]);

  return { restore, clear, lastSavedAt, isSaving };
}

/** 退出登录时清理所有草稿（WordPress 做法） */
export function clearAllDrafts(): void {
  try {
    for (let i = sessionStorage.length - 1; i >= 0; i--) {
      const key = sessionStorage.key(i);
      if (key && key.startsWith(DRAFT_PREFIX)) {
        sessionStorage.removeItem(key);
      }
    }
  } catch { /* 静默失败 */ }
}

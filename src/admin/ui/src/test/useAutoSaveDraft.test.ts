import { renderHook, act } from '@testing-library/react';
import { useAutoSaveDraft, clearAllDrafts } from '../hooks/useAutoSaveDraft';
import type { DraftData } from '../hooks/useAutoSaveDraft';

const DRAFT_PREFIX = 'colophon-autosave-post-';
const MAX_DRAFT_AGE_MS = 7 * 24 * 60 * 60 * 1000;

/** 构造一个已过期的假草稿数据 */
function buildExpiredDraftData(): DraftData {
  return {
    title: 'expired',
    content: 'should be cleaned',
    contentHtml: '<p>should be cleaned</p>',
    excerpt: '',
    categoryId: '',
    tagIds: [],
    savedAt: Date.now() - 8 * 24 * 60 * 60 * 1000,
  };
}

function getStorageKeysWithPrefix(prefix: string): string[] {
  const keys: string[] = [];
  for (let i = 0; i < sessionStorage.length; i++) {
    const k = sessionStorage.key(i);
    if (k && k.startsWith(prefix)) {
      keys.push(k);
    }
  }
  return keys;
}

beforeEach(() => {
  sessionStorage.clear();
  localStorage.clear();
});

describe('useAutoSaveDraft', () => {
  it('saves draft to sessionStorage after debounce, not to localStorage', () => {
    vi.useFakeTimers();
    const formData = { title: 'Hello', content: 'World', contentHtml: '<p>World</p>', excerpt: '', categoryId: '', tagIds: [] };
    const key = `${DRAFT_PREFIX}new`;

    const { result } = renderHook(() => useAutoSaveDraft(undefined, formData, true));

    // save 开始但尚未写入 (2 秒防抖)
    act(() => {
      vi.advanceTimersByTime(2000);
    });

    const raw = sessionStorage.getItem(key);
    expect(raw).not.toBeNull();

    const parsed: DraftData = JSON.parse(raw!);
    expect(parsed.title).toBe('Hello');
    expect(parsed.content).toBe('World');
    expect(parsed.savedAt).toBeGreaterThan(0);

    // localStorage 不应包含草稿数据
    const localRaw = localStorage.getItem(key);
    expect(localRaw).toBeNull();

    vi.useRealTimers();
  });

  it('does not save duplicate content (content fingerprint check)', () => {
    vi.useFakeTimers();
    const formData = { title: 'Same', content: 'Same', contentHtml: '<p>Same</p>', excerpt: '', categoryId: '', tagIds: [] };
    const key = `${DRAFT_PREFIX}new`;

    const { result, rerender } = renderHook(
      (props) => useAutoSaveDraft(undefined, props.formData, true),
      { initialProps: { formData } }
    );

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    const firstSavedAt: number = JSON.parse(sessionStorage.getItem(key)!).savedAt;

    // 用相同内容重新渲染 — 不应再次写入
    rerender({ formData: { ...formData } });

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    const secondSavedAt: number = JSON.parse(sessionStorage.getItem(key)!).savedAt;
    expect(secondSavedAt).toBe(firstSavedAt);

    vi.useRealTimers();
  });

  it('restore returns saved draft data', () => {
    vi.useFakeTimers();
    const formData = { title: 'Restored', content: 'Content', contentHtml: '<p>Content</p>', excerpt: 'ex', categoryId: 'cat1', tagIds: ['t1'] };

    const { result } = renderHook(() => useAutoSaveDraft(undefined, formData, true));

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    let restored: DraftData | null = null;
    act(() => {
      restored = result.current.restore();
    });

    expect(restored).not.toBeNull();
    expect(restored!.title).toBe('Restored');
    expect(restored!.content).toBe('Content');
    expect(restored!.excerpt).toBe('ex');
    expect(restored!.categoryId).toBe('cat1');
    expect(restored!.tagIds).toEqual(['t1']);

    vi.useRealTimers();
  });

  it('restore returns null when no draft saved', () => {
    const formData = { title: '', content: '', contentHtml: '', excerpt: '', categoryId: '', tagIds: [] };
    const { result } = renderHook(() => useAutoSaveDraft(undefined, formData, true));

    let restored: DraftData | null = null;
    act(() => {
      restored = result.current.restore();
    });

    expect(restored).toBeNull();
  });

  it('restore cleans expired draft and returns null', () => {
    const formData = { title: '', content: '', contentHtml: '', excerpt: '', categoryId: '', tagIds: [] };
    const key = `${DRAFT_PREFIX}new`;

    // 直接写入一个过期的草稿
    sessionStorage.setItem(key, JSON.stringify(buildExpiredDraftData()));

    const { result } = renderHook(() => useAutoSaveDraft(undefined, formData, true));

    let restored: DraftData | null = null;
    act(() => {
      restored = result.current.restore();
    });

    expect(restored).toBeNull();
    // 过期 key 已被从 sessionStorage 清除
    expect(sessionStorage.getItem(key)).toBeNull();
  });

  it('does not save when enabled is false', () => {
    vi.useFakeTimers();
    const formData = { title: 'Should Not Save', content: 'x', contentHtml: '<p>x</p>', excerpt: '', categoryId: '', tagIds: [] };
    const key = `${DRAFT_PREFIX}new`;

    renderHook(() => useAutoSaveDraft(undefined, formData, false));

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(sessionStorage.getItem(key)).toBeNull();
    vi.useRealTimers();
  });

  it('saves with postId-based key when postId is provided', () => {
    vi.useFakeTimers();
    const postId = 'post-42';
    const formData = { title: 'Post 42', content: 'body', contentHtml: '<p>body</p>', excerpt: '', categoryId: '', tagIds: [] };
    const expectedKey = `${DRAFT_PREFIX}${postId}`;

    const { result } = renderHook(() => useAutoSaveDraft(postId, formData, true));

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(sessionStorage.getItem(expectedKey)).not.toBeNull();
    vi.useRealTimers();
  });
});

describe('clearAllDrafts', () => {
  it('removes all draft keys from sessionStorage', () => {
    sessionStorage.setItem(`${DRAFT_PREFIX}post-1`, JSON.stringify({ title: 'draft 1', savedAt: Date.now() }));
    sessionStorage.setItem(`${DRAFT_PREFIX}post-2`, JSON.stringify({ title: 'draft 2', savedAt: Date.now() }));
    sessionStorage.setItem(`${DRAFT_PREFIX}new`, JSON.stringify({ title: 'new draft', savedAt: Date.now() }));
    // 一个非草稿 key，不应被清除
    sessionStorage.setItem('other-key', 'keep-me');

    clearAllDrafts();

    expect(getStorageKeysWithPrefix(DRAFT_PREFIX)).toHaveLength(0);
    // 非草稿 key 仍保留
    expect(sessionStorage.getItem('other-key')).toBe('keep-me');
  });

  it('clears drafts only, not other sessionStorage keys', () => {
    sessionStorage.setItem(`${DRAFT_PREFIX}x`, JSON.stringify({ savedAt: Date.now() }));
    sessionStorage.setItem('unrelated', 'value');

    clearAllDrafts();

    expect(sessionStorage.getItem(`${DRAFT_PREFIX}x`)).toBeNull();
    expect(sessionStorage.getItem('unrelated')).toBe('value');
  });
});

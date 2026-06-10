import type { Language } from './types';

const STORAGE_KEY = 'colophon_language';

/**
 * 检测用户语言偏好
 * 优先级：localStorage > navigator.language > 默认中文
 */
export function detectLanguage(): Language {
  // 1. 检查 localStorage
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved === 'zh' || saved === 'en') {
    return saved;
  }

  // 2. 检查浏览器语言
  const browserLang = navigator.language;
  if (browserLang.startsWith('en')) {
    return 'en';
  }

  // 3. 默认中文
  return 'zh';
}

/**
 * 保存语言偏好到 localStorage
 */
export function saveLanguage(lang: Language): void {
  localStorage.setItem(STORAGE_KEY, lang);
}

/**
 * 获取语言显示名称
 */
export function getLanguageName(lang: Language): string {
  return lang === 'zh' ? '简体中文' : 'English';
}

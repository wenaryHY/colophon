import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { apiData, setAccessToken, clearAccessToken, setOnAuthExpired, API_PREFIX } from '../lib/api';
import type { CurrentUser } from '../types';
import { useI18n } from '../i18n';
import { saveLanguage } from '../i18n/detector';
import { useToast } from './ToastContext';

interface RegisterData {
  username: string;
  email: string;
  password: string;
  display_name?: string;
  turnstile_token?: string | null;
}

/** 登录/注册响应中返回的用户摘要（id/username/role），完整用户信息通过 refreshUser() 获取 */
interface LoginResponse {
  user: { id: string; username: string; role: string };
  access_token: string;
}

interface AuthContextValue {
  user: CurrentUser | null;
  login: (login: string, password: string, rememberMe?: boolean, turnstileToken?: string | null) => Promise<{ success: boolean; message?: string }>;
  register: (data: RegisterData) => Promise<{ success: boolean; message?: string }>;
  logout: () => Promise<void>;
  refreshUser: () => Promise<void>;
  isLoading: boolean;
}

const AUTH_STORAGE_KEY = 'inkforge_auth_user';
const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<CurrentUser | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const { setLang, t } = useI18n();
  const toast = useToast();
  /** 仅首次挂载运行一次 auth 校验 */
  const authChecked = useRef(false);

  const clearAuth = useCallback(() => {
    clearAccessToken();
    setUser(null);
    try { sessionStorage.removeItem(AUTH_STORAGE_KEY); } catch { /* ignore quota / priv errors */ }
  }, []);

  /// 当 API 返回 401 且 refresh 失败时触发：清除登录态并 toast i18n 提示。
  /// 通过 ref 保持回调最新，避免主 effect 的 dependencies 漂移。
  const handleAuthExpiredRef = useRef<() => void>(() => {});
  useEffect(() => {
    handleAuthExpiredRef.current = () => {
      clearAuth();
      toast(t('permissionDeniedToAdmin'), 'error');
    };
  });

  const refreshUser = useCallback(async () => {
    const me = await apiData<CurrentUser>(`${API_PREFIX}/me`);
    setUser(me);
    try { sessionStorage.setItem(AUTH_STORAGE_KEY, JSON.stringify(me)); } catch { /* ignore */ }
    if (me.language) {
      setLang(me.language);
      saveLanguage(me.language);
    }
  }, [setLang]);

  useEffect(() => {
    if (authChecked.current) return;
    authChecked.current = true;

    // 快速恢复：从 sessionStorage 还原 user，消除首次骨架
    const stored = sessionStorage.getItem(AUTH_STORAGE_KEY);
    if (stored) {
      try {
        const parsed = JSON.parse(stored) as CurrentUser;
        setUser(parsed);
        setIsLoading(false);
      } catch { /* 脏数据，忽略 */ }
    }

    let active = true;
    setOnAuthExpired(() => handleAuthExpiredRef.current());
    refreshUser()
      .catch(() => {
        if (active) clearAuth();
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
      setOnAuthExpired(null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const login = useCallback(async (loginValue: string, password: string, rememberMe?: boolean, turnstileToken?: string | null) => {
    try {
      const data = await apiData<LoginResponse>(`${API_PREFIX}/auth/login`, {
        method: 'POST',
        body: JSON.stringify({ login: loginValue, password, remember_me: rememberMe, turnstile_token: turnstileToken ?? undefined }),
      });
      setAccessToken(data.access_token);
      await refreshUser();
      return { success: true };
    } catch (error) {
      clearAuth();
      return { success: false, message: error instanceof Error ? error.message : t('loginFailed') };
    }
  }, [clearAuth, refreshUser, t]);

  const register = useCallback(async (data: RegisterData) => {
    try {
      const result = await apiData<LoginResponse>(`${API_PREFIX}/auth/register`, {
        method: 'POST',
        body: JSON.stringify(data),
      });
      setAccessToken(result.access_token);
      await refreshUser();
      return { success: true };
    } catch (error) {
      clearAuth();
      return { success: false, message: error instanceof Error ? error.message : t('registerFailed') };
    }
  }, [clearAuth, refreshUser, t]);

  const logout = useCallback(async () => {
    try {
      await apiData(`${API_PREFIX}/auth/logout`, { method: 'POST' });
    } catch {
      // ignore server-side logout failure and always clear local auth state
    } finally {
      clearAuth();
    }
  }, [clearAuth]);

  const value = useMemo<AuthContextValue>(() => ({
    user,
    login,
    register,
    logout,
    refreshUser,
    isLoading,
  }), [user, login, register, logout, refreshUser, isLoading]);

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}

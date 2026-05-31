import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

/**
 * 实时预览上下文
 * 管理预览内容、主题、设备、缩放等状态，支持场景注册与自动同步
 */

// ==================== 类型定义 ====================

/** 内容类型 */
export type ContentType = 'markdown' | 'html' | 'zip';

/** 设备类型 */
export type DeviceType = 'desktop' | 'tablet' | 'mobile';

/** 预览状态 */
export interface PreviewState {
  /** 预览内容 */
  content: string;
  /** 内容类型 */
  contentType: ContentType;
  /** 当前主题 slug */
  theme: string;
  /** 主题配置 */
  themeConfig: Record<string, unknown>;
  /** 是否正在渲染 */
  isRendering: boolean;
  /** 渲染错误信息 */
  error: string | null;
  /** 设备模式 */
  device: DeviceType;
  /** 缩放比例 */
  zoom: number;
  /** 当前注册的场景 ID */
  sceneId: string | null;
}

/** 场景配置 - 场景需实现的接口 */
export interface SceneConfig {
  /** 获取当前内容 */
  getContent: () => string;
  /** 获取内容类型 */
  getContentType: () => ContentType;
  /** 获取主题（可选） */
  getTheme?: () => string;
  /** 获取主题配置（可选） */
  getThemeConfig?: () => Record<string, unknown>;
}

/** 预览上下文值（包含状态与操作方法） */
export interface PreviewContextType extends PreviewState {
  /** 设置预览内容和类型 */
  setContent: (content: string, type: ContentType) => void;
  /** 设置主题配置 */
  setThemeConfig: (config: Record<string, unknown>) => void;
  /** 设置设备模式 */
  setDevice: (device: DeviceType) => void;
  /** 设置缩放比例 */
  setZoom: (zoom: number) => void;
  /** 设置主题 */
  setTheme: (theme: string) => void;

  /** 注册预览场景 */
  registerScene: (id: string, config: SceneConfig) => void;
  /** 注销当前场景 */
  unregisterScene: () => void;

  /** 刷新预览 */
  refresh: () => void;
  /** 在新标签页中打开预览 */
  openInNewTab: () => void;
}

// ==================== 常量 ====================

/** localStorage 键名 - 主题 */
const THEME_STORAGE_KEY = 'inkforge_preview_theme';
/** localStorage 键名 - 设备模式 */
const DEVICE_STORAGE_KEY = 'inkforge_preview_device';
/** localStorage 键名 - 缩放比例 */
const ZOOM_STORAGE_KEY = 'inkforge_preview_zoom';
/** 默认主题 */
const DEFAULT_THEME = 'default';
/** 默认缩放比例 */
const DEFAULT_ZOOM = 1.0;
/** 最小缩放比例 */
const MIN_ZOOM = 0.25;
/** 最大缩放比例 */
const MAX_ZOOM = 2.0;
/** 缩放步进 */
const ZOOM_STEP = 0.25;

// ==================== 工具函数 ====================

/** 根据窗口宽度推断默认设备模式 */
function inferDefaultDevice(): DeviceType {
  if (typeof window === 'undefined') return 'desktop';
  const width = window.innerWidth;
  if (width < 768) return 'mobile';
  if (width < 1024) return 'tablet';
  return 'desktop';
}

/** 安全读取 localStorage 字符串值 */
function readStorage(key: string, fallback: string): string {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}

/** 安全读取 localStorage 数值 */
function readStorageNumber(key: string, fallback: number): number {
  try {
    const raw = localStorage.getItem(key);
    if (raw === null) return fallback;
    const num = Number(raw);
    return Number.isFinite(num) ? num : fallback;
  } catch {
    return fallback;
  }
}

/** 将缩放值限制在合法范围内 */
function clampZoom(zoom: number): number {
  return Math.round(Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, zoom)) * 100) / 100;
}

// ==================== Context ====================

const PreviewContext = createContext<PreviewContextType | null>(null);

// ==================== Provider ====================

export function PreviewProvider({ children }: { children: ReactNode }) {
  // ---- 状态初始化 ----
  const [content, setContentState] = useState('');
  const [contentType, setContentType] = useState<ContentType>('markdown');
  const [theme, setThemeState] = useState<string>(() =>
    readStorage(THEME_STORAGE_KEY, DEFAULT_THEME),
  );
  const [themeConfig, setThemeConfigState] = useState<Record<string, unknown>>({});
  const [isRendering, setIsRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [device, setDeviceState] = useState<DeviceType>(() =>
    readStorage(DEVICE_STORAGE_KEY, inferDefaultDevice()),
  );
  const [zoom, setZoomState] = useState<number>(() =>
    clampZoom(readStorageNumber(ZOOM_STORAGE_KEY, DEFAULT_ZOOM)),
  );
  const [sceneId, setSceneId] = useState<string | null>(null);

  // ---- 场景引用（避免 state 驱动的重渲染） ----
  const sceneConfigRef = useRef<SceneConfig | null>(null);
  const rafIdRef = useRef<number | null>(null);
  const mountedRef = useRef(true);

  // ---- 内容同步（基于 rAF 的轮询） ----
  const syncFromScene = useCallback(() => {
    if (!mountedRef.current) return;

    const config = sceneConfigRef.current;
    if (config) {
      try {
        const newContent = config.getContent();
        const newContentType = config.getContentType();
        setContentState(newContent);
        setContentType(newContentType);

        // 同步主题（如果场景提供了）
        if (config.getTheme) {
          const newTheme = config.getTheme();
          setThemeState(newTheme);
        }
        // 同步主题配置（如果场景提供了）
        if (config.getThemeConfig) {
          setThemeConfigState(config.getThemeConfig());
        }

        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : '场景内容获取失败');
      }
    }

    // 通过 rAF 调度下一次同步
    if (mountedRef.current && sceneConfigRef.current) {
      rafIdRef.current = requestAnimationFrame(syncFromScene);
    }
  }, []);

  // ---- 组件挂载/卸载 ----
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
      }
    };
  }, []);

  // ---- 响应式设备模式（监听窗口尺寸变化） ----
  useEffect(() => {
    const handleResize = () => {
      // 仅在未手动设置时自动切换（通过检查 localStorage 是否有用户显式设置）
      try {
        if (!localStorage.getItem(DEVICE_STORAGE_KEY)) {
          setDeviceState(inferDefaultDevice());
        }
      } catch {
        // ignore
      }
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // ---- 操作方法 ----

  /** 设置预览内容和类型 */
  const setContent = useCallback((newContent: string, type: ContentType) => {
    setContentState(newContent);
    setContentType(type);
    setError(null);
  }, []);

  /** 设置主题 */
  const setTheme = useCallback((newTheme: string) => {
    setThemeState(newTheme);
    try {
      localStorage.setItem(THEME_STORAGE_KEY, newTheme);
    } catch {
      // ignore
    }
  }, []);

  /** 设置主题配置 */
  const setThemeConfig = useCallback((config: Record<string, unknown>) => {
    setThemeConfigState(config);
  }, []);

  /** 设置设备模式 */
  const setDevice = useCallback((newDevice: DeviceType) => {
    setDeviceState(newDevice);
    try {
      localStorage.setItem(DEVICE_STORAGE_KEY, newDevice);
    } catch {
      // ignore
    }
  }, []);

  /** 设置缩放比例 */
  const setZoom = useCallback((newZoom: number) => {
    const clamped = clampZoom(newZoom);
    setZoomState(clamped);
    try {
      localStorage.setItem(ZOOM_STORAGE_KEY, String(clamped));
    } catch {
      // ignore
    }
  }, []);

  /** 注册预览场景 */
  const registerScene = useCallback(
    (id: string, config: SceneConfig) => {
      // 如果已有场景在运行，先停止 rAF 循环
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }

      setSceneId(id);
      sceneConfigRef.current = config;
      setIsRendering(true);

      // 立即同步一次内容
      try {
        const newContent = config.getContent();
        const newContentType = config.getContentType();
        setContentState(newContent);
        setContentType(newContentType);

        if (config.getTheme) {
          const newTheme = config.getTheme();
          setThemeState(newTheme);
          try {
            localStorage.setItem(THEME_STORAGE_KEY, newTheme);
          } catch {
            // ignore
          }
        }
        if (config.getThemeConfig) {
          setThemeConfigState(config.getThemeConfig());
        }

        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : '场景注册时获取内容失败');
      } finally {
        setIsRendering(false);
      }

      // 启动 rAF 同步循环
      rafIdRef.current = requestAnimationFrame(syncFromScene);
    },
    [syncFromScene],
  );

  /** 注销当前场景 */
  const unregisterScene = useCallback(() => {
    if (rafIdRef.current !== null) {
      cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }
    sceneConfigRef.current = null;
    setSceneId(null);
    setIsRendering(false);
  }, []);

  /** 刷新预览 - 重新从场景获取内容 */
  const refresh = useCallback(() => {
    const config = sceneConfigRef.current;
    if (!config) return;

    setIsRendering(true);
    try {
      const newContent = config.getContent();
      const newContentType = config.getContentType();
      setContentState(newContent);
      setContentType(newContentType);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : '刷新预览失败');
    } finally {
      setIsRendering(false);
    }
  }, []);

  /** 在新标签页中打开预览 */
  const openInNewTab = useCallback(() => {
    // 将内容编码到 URL 参数中打开新标签页
    const params = new URLSearchParams({
      content: content,
      contentType: contentType,
      theme: theme,
      device: device,
      zoom: String(zoom),
    });

    // 如果有主题配置，序列化后加入
    if (Object.keys(themeConfig).length > 0) {
      try {
        params.set('themeConfig', JSON.stringify(themeConfig));
      } catch {
        // ignore 序列化失败
      }
    }

    // 构造预览 URL（假设存在 /preview 路由）
    const previewUrl = `/preview?${params.toString()}`;
    window.open(previewUrl, '_blank');
  }, [content, contentType, theme, themeConfig, device, zoom]);

  // ---- Context 值 ----
  const value = useMemo<PreviewContextType>(
    () => ({
      // 状态
      content,
      contentType,
      theme,
      themeConfig,
      isRendering,
      error,
      device,
      zoom,
      sceneId,
      // 更新方法
      setContent,
      setTheme,
      setThemeConfig,
      setDevice,
      setZoom,
      // 场景管理
      registerScene,
      unregisterScene,
      // 操作
      refresh,
      openInNewTab,
    }),
    [
      content,
      contentType,
      theme,
      themeConfig,
      isRendering,
      error,
      device,
      zoom,
      sceneId,
      setContent,
      setTheme,
      setThemeConfig,
      setDevice,
      setZoom,
      registerScene,
      unregisterScene,
      refresh,
      openInNewTab,
    ],
  );

  return <PreviewContext.Provider value={value}>{children}</PreviewContext.Provider>;
}

// ==================== Hook ====================

/**
 * 获取预览上下文
 * 必须在 PreviewProvider 内部使用
 */
export function usePreview(): PreviewContextType {
  const ctx = useContext(PreviewContext);
  if (!ctx) {
    throw new Error('usePreview must be used within PreviewProvider');
  }
  return ctx;
}

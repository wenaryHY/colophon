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
 * 管理预览状态（设备、缩放），支持场景注册
 * 内容渲染改为后端 API 驱动，由 PreviewRenderer 负责 fetch
 */

// ==================== 类型定义 ====================

/** 设备类型 */
export type DeviceType = 'desktop' | 'tablet' | 'mobile';

/** 预览状态 */
export interface PreviewState {
  /** 设备模式 */
  device: DeviceType;
  /** 缩放比例 */
  zoom: number;
  /** 当前注册的场景 ID */
  sceneId: string | null;
  /** 刷新计数，用于触发 PreviewRenderer 重新 fetch */
  refreshKey: number;
}

/** 场景配置 - 场景需实现的接口 */
export interface SceneConfig {
  /** 获取预览请求参数（传给后端 API） */
  getRequestParams: () => {
    content: string;
    content_type: string;
  };
  /** 获取主题渲染参数（可选） */
  getThemeParams?: () => {
    theme_slug: string;
    theme_config?: string;
  };
  /** 内容变化回调（可选，用于实时更新） */
  onChange?: (callback: () => void) => void;
}

/** 预览上下文值（包含状态与操作方法） */
export interface PreviewContextType extends PreviewState {
  /** 设置设备模式 */
  setDevice: (device: DeviceType) => void;
  /** 设置缩放比例 */
  setZoom: (zoom: number) => void;

  /** 注册预览场景 */
  registerScene: (id: string, config: SceneConfig) => void;
  /** 注销当前场景 */
  unregisterScene: () => void;

  /** 获取当前请求参数（用于后端 API 调用） */
  getRequestParams: () => { content: string; content_type: string } | null;
  /** 获取当前主题参数 */
  getThemeParams: () => { theme_slug: string; theme_config?: string } | null;

  /** 刷新预览 */
  refresh: () => void;
  /** 在新标签页中打开预览 */
  openInNewTab: (mode?: 'content' | 'theme') => void;
}

// ==================== 常量 ====================

/** localStorage 键名 - 设备模式 */
const DEVICE_STORAGE_KEY = 'inkforge_preview_device';
/** localStorage 键名 - 缩放比例 */
const ZOOM_STORAGE_KEY = 'inkforge_preview_zoom';
/** 默认缩放比例 */
const DEFAULT_ZOOM = 1.0;
/** 最小缩放比例 */
const MIN_ZOOM = 0.25;
/** 最大缩放比例 */
const MAX_ZOOM = 2.0;

// ==================== 工具函数 ====================

/** 根据窗口宽度推断默认设备模式 */
function inferDefaultDevice(): DeviceType {
  if (typeof window === 'undefined') return 'desktop';
  const width = window.innerWidth;
  if (width < 768) return 'mobile';
  if (width < 1024) return 'tablet';
  return 'desktop';
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
  const [device, setDeviceState] = useState<DeviceType>(() => {
    try {
      const saved = localStorage.getItem(DEVICE_STORAGE_KEY);
      if (saved === 'desktop' || saved === 'tablet' || saved === 'mobile') return saved;
    } catch {
      // ignore
    }
    return inferDefaultDevice();
  });
  const [zoom, setZoomState] = useState<number>(() =>
    clampZoom(readStorageNumber(ZOOM_STORAGE_KEY, DEFAULT_ZOOM)),
  );
  const [sceneId, setSceneId] = useState<string | null>(null);

  // ---- 刷新计数（用于触发 PreviewRenderer 重新 fetch） ----
  const [refreshKey, setRefreshKey] = useState(0);

  // ---- 场景引用 ----
  const sceneConfigRef = useRef<SceneConfig | null>(null);
  const onSceneUnregisterRef = useRef<(() => void) | null>(null);

  // ---- 响应式设备模式（监听窗口尺寸变化） ----
  useEffect(() => {
    const handleResize = () => {
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

  /** 从当前场景获取请求参数 */
  const getRequestParams = useCallback((): { content: string; content_type: string } | null => {
    const config = sceneConfigRef.current;
    if (!config) return null;
    try {
      return config.getRequestParams();
    } catch {
      return null;
    }
  }, []);

  /** 从当前场景获取主题参数 */
  const getThemeParams = useCallback((): { theme_slug: string; theme_config?: string } | null => {
    const config = sceneConfigRef.current;
    if (!config || !config.getThemeParams) return null;
    try {
      return config.getThemeParams();
    } catch {
      return null;
    }
  }, []);

  /** 注册预览场景 */
  const registerScene = useCallback(
    (id: string, config: SceneConfig) => {
      // 注销前一个场景的回调
      if (onSceneUnregisterRef.current) {
        try { onSceneUnregisterRef.current(); } catch { /* ignore */ }
      }

      setSceneId(id);
      sceneConfigRef.current = config;

      // 注册 onChange 回调，用于触发刷新
      config.onChange?.(() => {
        setRefreshKey((k) => k + 1);
      });

      // 向场景提供取消注册的回调
      onSceneUnregisterRef.current = () => {
        // 预留钩子
      };
    },
    [],
  );

  /** 注销当前场景 */
  const unregisterScene = useCallback(() => {
    if (onSceneUnregisterRef.current) {
      try { onSceneUnregisterRef.current(); } catch { /* ignore */ }
      onSceneUnregisterRef.current = null;
    }
    sceneConfigRef.current = null;
    setSceneId(null);
  }, []);

  /** 刷新预览 - 触发 PreviewRenderer 重新 fetch */
  const refresh = useCallback(() => {
    setRefreshKey((k) => k + 1);
  }, []);

  /** 在新标签页中打开预览 */
  const openInNewTab = useCallback((mode: 'content' | 'theme' = 'content') => {
    const params = sceneConfigRef.current?.getRequestParams();
    const themeParams = sceneConfigRef.current?.getThemeParams?.();

    const previewData = {
      mode,
      content: params?.content || '',
      content_type: params?.content_type || 'post',
      theme_slug: themeParams?.theme_slug || '',
      theme_config: themeParams?.theme_config || '',
    };

    sessionStorage.setItem('inkforge-preview-params', JSON.stringify(previewData));
    window.open('/preview', '_blank');
  }, []);

  // ---- Context 值 ----
  const value = useMemo<PreviewContextType>(
    () => ({
      // 状态
      device,
      zoom,
      sceneId,
      refreshKey,
      // 更新方法
      setDevice,
      setZoom,
      // 场景管理
      registerScene,
      unregisterScene,
      // 参数获取
      getRequestParams,
      getThemeParams,
      // 操作
      refresh,
      openInNewTab,
    }),
    [
      device,
      zoom,
      sceneId,
      refreshKey,
      setDevice,
      setZoom,
      registerScene,
      unregisterScene,
      getRequestParams,
      getThemeParams,
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

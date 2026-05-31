import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { usePreview } from './PreviewContext';
import { Modal } from '../components/Modal';

// ==================== 类型定义 ====================

/** 渲染模式 */
type RenderMode = 'inline' | 'modal' | 'new-tab';

/** 预览渲染器属性 */
interface PreviewRendererProps {
  /** 渲染模式 */
  mode: RenderMode;
  /** 是否可见 */
  visible: boolean;
  /** 关闭回调（modal 模式） */
  onClose?: () => void;
  /** 预览容器样式 */
  className?: string;
}

/** iframe 消息格式 */
interface PreviewMessage {
  type: 'CONTENT_UPDATE' | 'THEME_UPDATE' | 'REFRESH';
  payload: {
    content?: string;
    contentType?: string;
    theme?: string;
    themeConfig?: Record<string, unknown>;
  };
}

// ==================== 常量 ====================

/** 预览 API 基础路径 */
const PREVIEW_API_BASE = '/api/v1/preview';

/** iframe sandbox 权限限制 */
const IFRAME_SANDBOX = 'allow-scripts allow-same-origin';

/** 消息同步防抖延迟（毫秒） */
const MESSAGE_DEBOUNCE_DELAY = 150;

// ==================== 工具函数 ====================

/**
 * 构建预览 URL
 * @param theme 主题 slug
 * @param themeConfig 主题配置
 * @returns 完整的预览 URL
 */
function buildPreviewUrl(theme: string, themeConfig: Record<string, unknown>): string {
  const params = new URLSearchParams();
  params.set('theme', theme);
  if (Object.keys(themeConfig).length > 0) {
    try {
      params.set('config', JSON.stringify(themeConfig));
    } catch {
      // 序列化失败时忽略配置参数
    }
  }
  return `${PREVIEW_API_BASE}?${params.toString()}`;
}

// ==================== 组件 ====================

/**
 * 预览渲染器组件
 * 使用 iframe 隔离渲染预览内容，支持 inline / modal / new-tab 三种模式
 *
 * @example
 * ```tsx
 * <PreviewRenderer
 *   mode="inline"
 *   visible={showPreview}
 *   onClose={() => setShowPreview(false)}
 * />
 * ```
 */
export function PreviewRenderer({
  mode,
  visible,
  onClose,
  className,
}: PreviewRendererProps) {
  const {
    content,
    contentType,
    theme,
    themeConfig,
    isRendering,
    error,
  } = usePreview();

  const iframeRef = useRef<HTMLIFrameElement>(null);
  const [iframeLoaded, setIframeLoaded] = useState(false);
  const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 预览 URL（memoize 避免不必要的重渲染）
  const previewUrl = useMemo(
    () => buildPreviewUrl(theme, themeConfig),
    [theme, themeConfig],
  );

  // ==================== 消息发送 ====================

  /**
   * 向 iframe 发送消息
   * @param message 要发送的消息
   */
  const sendMessageToIframe = useCallback((message: PreviewMessage) => {
    const iframe = iframeRef.current;
    if (!iframe?.contentWindow) return;

    try {
      iframe.contentWindow.postMessage(message, window.location.origin);
    } catch {
      // postMessage 失败时静默处理（跨域或 iframe 未加载）
    }
  }, []);

  // ==================== 内容同步 ====================

  /**
   * 同步内容到 iframe（带防抖）
   */
  const syncContent = useCallback(() => {
    if (!iframeLoaded || !visible) return;

    // 清除之前的防抖计时器
    if (debounceTimerRef.current) {
      clearTimeout(debounceTimerRef.current);
    }

    // 设置新的防抖计时器
    debounceTimerRef.current = setTimeout(() => {
      sendMessageToIframe({
        type: 'CONTENT_UPDATE',
        payload: {
          content,
          contentType,
        },
      });
    }, MESSAGE_DEBOUNCE_DELAY);
  }, [iframeLoaded, visible, content, contentType, sendMessageToIframe]);

  // 监听内容变化并同步
  useEffect(() => {
    syncContent();
    return () => {
      if (debounceTimerRef.current) {
        clearTimeout(debounceTimerRef.current);
      }
    };
  }, [syncContent]);

  // ==================== 主题同步 ====================

  useEffect(() => {
    if (!iframeLoaded || !visible) return;

    sendMessageToIframe({
      type: 'THEME_UPDATE',
      payload: {
        theme,
        themeConfig,
      },
    });
  }, [iframeLoaded, visible, theme, themeConfig, sendMessageToIframe]);

  // ==================== iframe 加载处理 ====================

  const handleIframeLoad = useCallback(() => {
    setIframeLoaded(true);
  }, []);

  // ==================== new-tab 模式处理 ====================

  useEffect(() => {
    if (mode !== 'new-tab' || !visible) return;

    const previewData = {
      content,
      contentType,
      theme,
      themeConfig,
    };
    
    sessionStorage.setItem('inkforge-preview-data', JSON.stringify(previewData));
    window.open('/preview', '_blank');

    // 打开后自动关闭（如果提供了 onClose）
    onClose?.();
  }, [mode, visible, content, contentType, theme, themeConfig, onClose]);

  // ==================== 渲染逻辑 ====================

  // new-tab 模式不渲染 DOM，只在 useEffect 中打开新标签
  if (mode === 'new-tab') {
    return null;
  }

  // 不可见时不渲染
  if (!visible) {
    return null;
  }

  // ==================== modal 模式 ====================

  if (mode === 'modal') {
    return (
      <Modal
        open={visible}
        onClose={onClose ?? (() => {})}
        title="预览"
        width="90%"
      >
        <div
          className={className}
          style={{
            position: 'relative',
            width: '100%',
            height: '70vh',
            minHeight: '400px',
            borderRadius: 'var(--radius-md)',
            overflow: 'hidden',
            border: '1px solid var(--md-outline-variant)',
            background: 'var(--md-surface-container-lowest)',
          }}
        >
          {/* 加载状态指示器 */}
          {isRendering && (
            <div
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                right: 0,
                height: '3px',
                background: 'linear-gradient(90deg, var(--md-primary) 25%, var(--md-primary-container) 50%, var(--md-primary) 75%)',
                backgroundSize: '200% 100%',
                zIndex: 10,
                animation: 'shimmer 1.5s ease-in-out infinite',
              }}
            />
          )}

          {/* 错误提示 */}
          {error && (
            <div
              style={{
                position: 'absolute',
                top: '50%',
                left: '50%',
                transform: 'translate(-50%, -50%)',
                padding: '16px 24px',
                background: 'var(--md-error-container)',
                color: 'var(--md-on-error-container)',
                borderRadius: 'var(--radius-md)',
                fontSize: '14px',
                zIndex: 10,
              }}
            >
              {error}
            </div>
          )}

          {/* 预览 iframe */}
          <iframe
            ref={iframeRef}
            src={previewUrl}
            sandbox={IFRAME_SANDBOX}
            loading="lazy"
            onLoad={handleIframeLoad}
            style={{
              width: '100%',
              height: '100%',
              border: 'none',
              background: 'var(--md-surface-container-lowest)',
            }}
            title="预览内容"
          />
        </div>
      </Modal>
    );
  }

  // ==================== inline 模式 ====================

  return (
    <div
      className={className}
      style={{
        position: 'relative',
        width: '50%',
        height: '100%',
        minHeight: '400px',
        borderLeft: '1px solid var(--md-outline-variant)',
        background: 'var(--md-surface-container-lowest)',
        flexShrink: 0,
      }}
    >
      {/* 加载状态指示器 */}
      {isRendering && (
        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            right: 0,
            height: '3px',
            background: 'linear-gradient(90deg, var(--md-primary) 25%, var(--md-primary-container) 50%, var(--md-primary) 75%)',
            backgroundSize: '200% 100%',
            zIndex: 10,
            animation: 'shimmer 1.5s ease-in-out infinite',
          }}
        />
      )}

      {/* 错误提示 */}
      {error && (
        <div
          style={{
            position: 'absolute',
            top: '50%',
            left: '50%',
            transform: 'translate(-50%, -50%)',
            padding: '16px 24px',
            background: 'var(--md-error-container)',
            color: 'var(--md-on-error-container)',
            borderRadius: 'var(--radius-md)',
            fontSize: '14px',
            zIndex: 10,
          }}
        >
          {error}
        </div>
      )}

      {/* 预览 iframe */}
      <iframe
        ref={iframeRef}
        src={previewUrl}
        sandbox={IFRAME_SANDBOX}
        loading="lazy"
        onLoad={handleIframeLoad}
        style={{
          width: '100%',
          height: '100%',
          border: 'none',
          background: 'var(--md-surface-container-lowest)',
        }}
        title="预览内容"
      />
    </div>
  );
}

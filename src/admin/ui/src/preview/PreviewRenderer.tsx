import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { usePreview, SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB } from './PreviewContext';
import { Modal } from '../components/Modal';
import { calculatePopoverPositionRelativeToFabButtonRect } from '../fab/usePopoverPosition';

// ==================== 类型定义 ====================

/** 渲染模式 */
type RenderMode = 'inline' | 'modal' | 'new-tab' | 'fab-popover';

/** 后端预览请求参数 */
interface PreviewRequestParams {
  content: string;
  content_type: string;
}

/** 主题预览请求参数 */
interface ThemePreviewParams {
  theme_slug: string;
  theme_config?: string;
}

/** 预览渲染器属性 */
interface PreviewRendererProps {
  /** 渲染模式 */
  mode: RenderMode;
  /** 是否可见 */
  visible: boolean;
  /** 关闭回调（modal / fab-popover 模式） */
  onClose?: () => void;
  /** 预览容器样式 */
  className?: string;
  /** FAB 按钮的屏幕坐标（fab-popover 模式用于定位浮窗） */
  fabRect?: DOMRect | null;
}

// ==================== 常量 ====================

/** 防抖延迟（ms） */
const DEBOUNCE_DELAY = 300;

// ==================== Hook: 防抖 fetch 预览 ====================

/**
 * 向后端 API 发起预览请求，返回渲染后的 HTML
 * 内置 300ms 防抖，避免频繁请求
 * 根据 mode 切换请求端点：content → /api/v1/preview/content，theme → /api/v1/preview/theme
 * @param mode - 预览模式（内容渲染 / 主题渲染）
 * @param params - 内容预览请求参数
 * @param themeParams - 主题预览请求参数
 * @param refreshKey - 刷新计数，变化时强制重新 fetch
 */
function usePreviewFetch(
  mode: 'content' | 'theme',
  params: PreviewRequestParams | null,
  themeParams: ThemePreviewParams | null,
  refreshKey: number,
) {
  const [html, setHtml] = useState('');
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // 根据 mode 判断是否有有效参数
    const hasValidParams =
      mode === 'content'
        ? !!(params && params.content)
        : !!(themeParams && themeParams.theme_slug);

    if (!hasValidParams) {
      setHtml('');
      setLoading(false);
      setError(null);
      return;
    }

    let cancelled = false;

    const timer = setTimeout(async () => {
      if (cancelled) return;

      setLoading(true);
      setError(null);
      try {
        const formData = new URLSearchParams();
        let endpoint: string;

        if (mode === 'content' && params) {
          formData.append('content', params.content);
          formData.append('content_type', params.content_type);
          endpoint = '/api/v1/preview/content';
        } else if (mode === 'theme' && themeParams) {
          // 始终发送 content（后端 preview_theme handler 要求 content 必填）
          if (params) {
            formData.append('content', params.content);
            formData.append('content_type', params.content_type);
          }
          formData.append('theme_slug', themeParams.theme_slug);
          if (themeParams.theme_config) {
            formData.append('theme_config', themeParams.theme_config);
          }
          endpoint = '/api/v1/preview/theme';
        } else {
          return;
        }

        const resp = await fetch(endpoint, {
          method: 'POST',
          body: formData,
          credentials: 'include',
        });
        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
        const text = await resp.text();
        if (!cancelled) {
          setHtml(text);
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : 'Unknown error');
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }, DEBOUNCE_DELAY);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [
    mode,
    params?.content,
    params?.content_type,
    themeParams?.theme_slug,
    themeParams?.theme_config,
    refreshKey,
  ]);

  return { html, loading, error };
}

// ==================== 组件 ====================

/**
 * 预览渲染器组件
 * 使用后端 API 渲染内容，iframe 展示返回的完整 HTML
 * 支持 inline / modal / new-tab / fab-popover 四种模式
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
  fabRect,
}: PreviewRendererProps) {
  const { getRequestParams, getThemeParams, openInNewTab, refreshKey } = usePreview();

  const iframeRef = useRef<HTMLIFrameElement>(null);

  // 预览模式切换：内容渲染 / 主题渲染
  const [previewMode, setPreviewMode] = useState<'content' | 'theme'>('content');

  // 获取当前场景的请求参数
  const requestParams = getRequestParams();
  const themeParams = getThemeParams();

  // 通过后端 API 获取渲染后的 HTML
  const { html, loading, error } = usePreviewFetch(
    previewMode,
    requestParams,
    themeParams,
    refreshKey,
  );

  // ==================== iframe 加载处理 ====================

  const handleIframeLoad = useCallback(() => {
    // iframe 加载完成，可在此做后续处理
  }, []);

  // ==================== 新标签页打开 ====================

  const handleOpenNewTab = useCallback(() => {
    openInNewTab(previewMode);
    onClose?.();
  }, [previewMode, openInNewTab, onClose]);

  // ==================== new-tab 模式处理 ====================

  const openInNewTabRef = useRef(openInNewTab);
  openInNewTabRef.current = openInNewTab;
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (mode !== 'new-tab' || !visible) return;
    // 清理可能残留的旧预览参数，确保新标签页读取到最新数据
    sessionStorage.removeItem(SESSION_STORAGE_KEY_FOR_PREVIEW_PARAMETERS_PASSED_TO_NEW_TAB);
    openInNewTabRef.current();
    onCloseRef.current?.();
  }, [mode, visible]);

  // ==================== 渲染逻辑 ====================

  // new-tab 模式不渲染 DOM，只在 useEffect 中打开新标签
  if (mode === 'new-tab') {
    return null;
  }

  // 不可见时不渲染
  if (!visible) {
    return null;
  }

  // ==================== fab-popover 模式 ====================

  if (mode === 'fab-popover') {
    // 移动端自适应尺寸
    const isMobile = typeof window !== 'undefined' && window.innerWidth < 768;
    const popoverWidth = isMobile ? 'calc(100vw - 32px)' : 420;
    const popoverHeight = isMobile ? 'calc(100vh - 120px)' : 500;

    // 位置计算使用数值
    const popoverWidthNum = isMobile ? window.innerWidth - 32 : 420;
    const popoverHeightNum = isMobile ? window.innerHeight - 120 : 500;

    const popoverPos = calculatePopoverPositionRelativeToFabButtonRect(
      fabRect ?? null,
      popoverWidthNum,
      popoverHeightNum,
    );

    return createPortal(
      <div
        className={className}
        style={{
          position: 'fixed',
          left: popoverPos.x,
          top: popoverPos.y,
          width: popoverWidth,
          height: popoverHeight,
          borderRadius: 'var(--radius-lg)',
          overflow: 'hidden',
          border: '1px solid var(--md-outline-variant)',
          background: 'var(--md-surface-container-lowest)',
          boxShadow: 'var(--elevation-3)',
          zIndex: 998,
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        {/* 顶部栏 */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '8px 16px',
            borderBottom: '1px solid var(--md-outline-variant)',
            flexShrink: 0,
            background: 'var(--md-surface-container-low)',
          }}
        >
          <span
            style={{
              fontSize: '13px',
              fontWeight: 600,
              color: 'var(--md-on-surface-variant)',
              flexShrink: 0,
            }}
          >
            预览
          </span>

          {/* 工具栏：模式切换 + 新标签页 */}
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            {/* 模式切换按钮 */}
            <div style={{ display: 'flex', gap: 4 }}>
              <button
                onClick={() => setPreviewMode('content')}
                style={{
                  padding: '4px 10px',
                  borderRadius: 'var(--radius-full)',
                  fontSize: '11px',
                  fontWeight: previewMode === 'content' ? 600 : 400,
                  background:
                    previewMode === 'content'
                      ? 'var(--md-primary)'
                      : 'var(--md-surface-container)',
                  color:
                    previewMode === 'content'
                      ? 'var(--md-on-primary)'
                      : 'var(--md-on-surface-variant)',
                  border: 'none',
                  cursor: 'pointer',
                }}
              >
                内容
              </button>
              <button
                onClick={() => setPreviewMode('theme')}
                style={{
                  padding: '4px 10px',
                  borderRadius: 'var(--radius-full)',
                  fontSize: '11px',
                  fontWeight: previewMode === 'theme' ? 600 : 400,
                  background:
                    previewMode === 'theme'
                      ? 'var(--md-primary)'
                      : 'var(--md-surface-container)',
                  color:
                    previewMode === 'theme'
                      ? 'var(--md-on-primary)'
                      : 'var(--md-on-surface-variant)',
                  border: 'none',
                  cursor: 'pointer',
                }}
              >
                主题
              </button>
            </div>

            {/* 新标签页打开按钮 */}
            <button
              onClick={handleOpenNewTab}
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: 4,
                padding: '4px 10px',
                borderRadius: 'var(--radius-full)',
                fontSize: '11px',
                border: 'none',
                cursor: 'pointer',
                background: 'var(--md-surface-container)',
                color: 'var(--md-on-surface-variant)',
                flexShrink: 0,
              }}
            >
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
                <polyline points="15 3 21 3 21 9" />
                <line x1="10" y1="14" x2="21" y2="3" />
              </svg>
              新标签页
            </button>
          </div>

          {/* 关闭按钮 */}
          <button
            onClick={onClose}
            style={{
              width: 28,
              height: 28,
              borderRadius: 'var(--radius-full)',
              border: 'none',
              background: 'transparent',
              color: 'var(--md-on-surface-variant)',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: '16px',
              transition: 'background 0.2s',
              flexShrink: 0,
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--md-surface-container-highest)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent';
            }}
            aria-label="关闭预览"
          >
            ✕
          </button>
        </div>

        {/* 加载/错误/内容 */}
        {renderIframe({
          html,
          loading,
          error,
          iframeRef,
          onLoad: handleIframeLoad,
          style: { flex: 1, minHeight: 0 },
        })}
      </div>,
      document.body,
    );
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
          {renderIframe({
            html,
            loading,
            error,
            iframeRef,
            onLoad: handleIframeLoad,
            style: { width: '100%', height: '100%' },
          })}
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
      {renderIframe({
        html,
        loading,
        error,
        iframeRef,
        onLoad: handleIframeLoad,
        style: { width: '100%', height: '100%' },
      })}
    </div>
  );
}

// ==================== 辅助渲染函数 ====================

interface RenderIframeOptions {
  html: string;
  loading: boolean;
  error: string | null;
  iframeRef: React.RefObject<HTMLIFrameElement | null>;
  onLoad: () => void;
  style: React.CSSProperties;
}

/**
 * 渲染 iframe 预览容器（含 loading / error 状态）
 * 三种模式（inline / modal / fab-popover）共用此渲染逻辑
 */
function renderIframe({
  html,
  loading,
  error,
  iframeRef,
  onLoad,
  style,
}: RenderIframeOptions) {
  return (
    <div style={{ position: 'relative', ...style }}>
      {/* 加载状态 */}
      {loading && (
        <div
          style={{
            position: 'absolute',
            top: 0,
            left: 0,
            right: 0,
            height: '3px',
            background:
              'linear-gradient(90deg, var(--md-primary) 25%, var(--md-primary-container) 50%, var(--md-primary) 75%)',
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
        srcDoc={html}
        sandbox="allow-scripts allow-same-origin"
        onLoad={onLoad}
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

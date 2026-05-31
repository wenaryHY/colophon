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

// ==================== 工具函数 ====================

/**
 * 构建预览页面的完整 HTML（用于 iframe srcdoc）
 */
function buildPreviewHtml(
  content: string,
  contentType: string,
  _theme: string,
  _themeConfig: Record<string, unknown>,
): string {
  // 基础样式
  const baseStyles = `
    * { margin: 0; padding: 0; box-sizing: border-box; }
    body {
      font-family: system-ui, -apple-system, sans-serif;
      line-height: 1.8;
      padding: 24px;
      max-width: 800px;
      margin: 0 auto;
      color: #333;
      background: #fff;
    }
    h1, h2, h3 { margin-top: 1.5em; margin-bottom: 0.5em; }
    p { margin-bottom: 1em; }
    img { max-width: 100%; height: auto; }
    pre { background: #f5f5f5; padding: 16px; border-radius: 8px; overflow-x: auto; }
    code { background: #f5f5f5; padding: 2px 6px; border-radius: 4px; font-size: 0.9em; }
    blockquote { border-left: 3px solid #ddd; padding-left: 16px; margin-left: 0; color: #666; }
    table { border-collapse: collapse; width: 100%; }
    th, td { border: 1px solid #ddd; padding: 8px 12px; text-align: left; }
    .preview-empty { text-align: center; color: #999; padding: 40px; font-size: 16px; }
  `;

  // 渲染内容
  let bodyContent = '';
  if (!content) {
    bodyContent = '<div class="preview-empty">暂无内容</div>';
  } else if (contentType === 'html') {
    bodyContent = content;
  } else if (contentType === 'markdown') {
    bodyContent = simpleMarkdownToHtml(content);
  } else {
    bodyContent = `<pre>${escapeHtml(content)}</pre>`;
  }

  return `<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>预览</title>
  <style>${baseStyles}</style>
</head>
<body>
  ${bodyContent}
  <script>
    // 监听父页面消息，动态更新内容
    window.addEventListener('message', function(e) {
      if (e.data && e.data.type === 'CONTENT_UPDATE') {
        document.body.innerHTML = e.data.payload.html || e.data.payload.content || '';
      }
    });
  </script>
</body>
</html>`;
}

/** 简单的 Markdown 转 HTML */
function simpleMarkdownToHtml(md: string): string {
  let html = md;
  // 标题
  html = html.replace(/^#### (.+)$/gm, '<h4>$1</h4>');
  html = html.replace(/^### (.+)$/gm, '<h3>$1</h3>');
  html = html.replace(/^## (.+)$/gm, '<h2>$1</h2>');
  html = html.replace(/^# (.+)$/gm, '<h1>$1</h1>');
  // 粗体/斜体
  html = html.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>');
  html = html.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
  html = html.replace(/\*(.+?)\*/g, '<em>$1</em>');
  // 代码块
  html = html.replace(/```(\w*)\n([\s\S]*?)```/g, '<pre><code>$2</code></pre>');
  // 行内代码
  html = html.replace(/`(.+?)`/g, '<code>$1</code>');
  // 链接
  html = html.replace(/\[(.+?)\]\((.+?)\)/g, '<a href="$2">$1</a>');
  // 图片
  html = html.replace(/!\[(.+?)\]\((.+?)\)/g, '<img src="$2" alt="$1">');
  // 引用
  html = html.replace(/^> (.+)$/gm, '<blockquote>$1</blockquote>');
  // 段落
  html = html.replace(/^(?!<[a-z]|$)(.+)$/gm, '<p>$1</p>');
  // 空行
  html = html.replace(/\n\n/g, '<br>');

  return html;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

// ==================== 组件 ====================

/**
 * 预览渲染器组件
 * 使用 iframe srcdoc 在前端直接渲染预览内容，无需后端 API
 * 支持 inline / modal / new-tab 三种模式
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

  // 构建 srcdoc HTML
  const srcDoc = useMemo(
    () => buildPreviewHtml(content, contentType, theme, themeConfig),
    [content, contentType, theme, themeConfig],
  );

  // ==================== postMessage 增量更新 ====================

  // 内容变化时通过 postMessage 增量更新（避免整个 iframe 重载）
  useEffect(() => {
    if (iframeRef.current?.contentWindow && iframeLoaded) {
      const html = contentType === 'markdown'
        ? simpleMarkdownToHtml(content)
        : content;
      iframeRef.current.contentWindow.postMessage({
        type: 'CONTENT_UPDATE',
        payload: { html, content, contentType },
      }, '*');
    }
  }, [content, contentType, iframeLoaded]);

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

          {/* 预览 iframe（srcdoc 方式） */}
          <iframe
            ref={iframeRef}
            srcDoc={srcDoc}
            sandbox="allow-scripts"
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

      {/* 预览 iframe（srcdoc 方式） */}
      <iframe
        ref={iframeRef}
        srcDoc={srcDoc}
        sandbox="allow-scripts"
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

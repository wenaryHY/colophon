/**
 * InkForge comment frontend module.
 */

type ToastType = 'success' | 'error' | 'info';

/** 读取当前页面语言，从 <html lang> 属性获取 */
function getPageLang(): 'zh' | 'en' {
  const langAttr = document.documentElement.lang || '';
  return langAttr.startsWith('zh') ? 'zh' : 'en';
}

const MSG: Record<string, Record<string, string>> = {
  zh: {
    commentApproved: '评论已发布。',
    commentPending: '评论已提交审核，通过后显示。',
    submitError: '无法提交评论。',
    newCommentAppeared: '有一条新评论已发布。',
  },
  en: {
    commentApproved: 'Comment published.',
    commentPending: 'Comment submitted for review. It will appear after approval.',
    submitError: 'Unable to submit comment.',
    newCommentAppeared: 'A new approved comment just appeared.',
  },
};

function t(key: string): string {
  return MSG[getPageLang()]?.[key] || MSG.en[key] || key;
}

function logDebug(event: string, details?: Record<string, unknown>): void {
  console.debug('[InkForge][comments][debug]', event, details || {});
}

function logError(event: string, details?: Record<string, unknown>): void {
  console.error('[InkForge][comments][error]', event, details || {});
}

declare global {
  interface Window {
    __POST_DATA__?: {
      id?: string;
      slug?: string;
    };
    InkForgeApi: {
      apiRequest<T>(path: string, options?: RequestInit & { body?: unknown }): Promise<T>;
      sanitizeRedirect(target: string | null | undefined, fallback?: string): string;
    };
    initComments?: () => void;
  }
}

function showToast(message: string, type: ToastType = 'info'): void {
  let container = document.getElementById('toast-container');
  if (!container) {
    container = document.createElement('div');
    container.id = 'toast-container';
    Object.assign(container.style, {
      position: 'fixed',
      top: '24px',
      right: '24px',
      zIndex: '9999',
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
    });
    document.body.appendChild(container);
  }

  const toast = document.createElement('div');
  const colors: Record<ToastType, string> = {
    success: '#10b981',
    error: '#ef4444',
    info: '#3b82f6',
  };

  Object.assign(toast.style, {
    background: colors[type],
    color: '#fff',
    padding: '12px 20px',
    borderRadius: '8px',
    fontSize: '14px',
    boxShadow: '0 4px 12px rgba(0,0,0,.15)',
    opacity: '0',
    transform: 'translateX(40px)',
    transition: 'all .3s ease',
    maxWidth: '360px',
  });

  toast.textContent = message;
  container.appendChild(toast);

  requestAnimationFrame(() => {
    toast.style.opacity = '1';
    toast.style.transform = 'translateX(0)';
  });

  setTimeout(() => {
    toast.style.opacity = '0';
    toast.style.transform = 'translateX(40px)';
    setTimeout(() => toast.remove(), 300);
  }, 4000);
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

async function submitComment(event: Event): Promise<void> {
  event.preventDefault();
  const form = event.target as HTMLFormElement;
  const data = new FormData(form);
  const slug = window.__POST_DATA__?.slug || '';
  logDebug('submit_start', { slug });

  try {
    const result = await window.InkForgeApi.apiRequest(`/api/v1/posts/${slug}/comments`, {
      method: 'POST',
      body: {
        content: String(data.get('content') || '').trim(),
      },
    });

    if (result && result.status === 'approved') {
      showToast(t('commentApproved'), 'success');
    } else {
      showToast(t('commentPending'), 'info');
    }
    logDebug('submit_success', { slug });
    form.reset();
  } catch (error) {
    const requestError = error as Error & { status?: number };
    if (requestError.status === 401) {
      const target = window.InkForgeApi.sanitizeRedirect(window.location.pathname + window.location.search, '/');
      logDebug('submit_redirect_login', { slug, target });
      window.location.href = `/login?redirect=${encodeURIComponent(target)}`;
      return;
    }

    logError('submit_error', {
      slug,
      message: requestError.message,
      status: requestError.status,
    });
    showToast(requestError.message || t('submitError'), 'error');
  }
}

function initCommentWebSocket(): void {
  const postId = window.__POST_DATA__?.id;
  if (!postId) return;

  const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
  let reconnectDelay = 1000;

  function connect(): void {
    logDebug('ws_connect_attempt', { postId });
    const ws = new WebSocket(
      `${protocol}//${location.host}/ws/public?post_id=${encodeURIComponent(postId)}`
    );

    ws.onopen = () => {
      logDebug('ws_open', { postId });
      reconnectDelay = 1000;
    };

    ws.onmessage = (messageEvent) => {
      try {
        const event = JSON.parse(messageEvent.data) as {
          type?: string;
          data?: { post_id?: string; author_name?: string; created_at?: string; content?: string };
        };

        if (event.type === 'comment_approved' && event.data?.post_id === postId) {
          const list = document.getElementById('comment-list');
          const countEl = document.getElementById('comment-count');

          if (list && event.data) {
            const item = document.createElement('div');
            item.className = 'comment';
            item.style.cssText = 'animation:fadeIn .4s ease;';

            const name = event.data.author_name || '?';
            const avatar = name.charAt(0).toUpperCase();
            const time = (event.data.created_at || '').substring(0, 16);
            item.innerHTML = `
              <div class="comment-header">
                <div class="comment-avatar">${escapeHtml(avatar)}</div>
                <div>
                  <span class="comment-author">${escapeHtml(name)}</span>
                  <span class="comment-time">${escapeHtml(time)}</span>
                </div>
              </div>
              <p class="comment-body">${escapeHtml(event.data.content || '')}</p>
            `;
            list.appendChild(item);
          }

          if (countEl) {
            const current = parseInt(countEl.textContent || '0', 10) || 0;
            countEl.textContent = String(current + 1);
          }

          showToast(t('newCommentAppeared'), 'info');
          logDebug('ws_comment_approved', { postId });
        }
      } catch (error) {
        logError('ws_parse_error', { postId, error });
      }
    };

    ws.onclose = () => {
      logDebug('ws_close', { postId, reconnectDelay });
      setTimeout(connect, reconnectDelay);
      reconnectDelay = Math.min(reconnectDelay * 2, 30000);
    };

    ws.onerror = () => {
      logError('ws_error', { postId });
      return undefined;
    };
  }

  connect();
}

function initComments(): void {
  const form = document.querySelector('.comment-form');
  logDebug('init', { hasForm: Boolean(form), postId: window.__POST_DATA__?.id || null });
  if (form) {
    form.addEventListener('submit', submitComment);
  }

  initCommentWebSocket();

  if (!document.getElementById('inkforge-fadein-style')) {
    const style = document.createElement('style');
    style.id = 'inkforge-fadein-style';
    style.textContent =
      '@keyframes fadeIn { from { opacity:0; transform:translateY(10px); } to { opacity:1; transform:translateY(0); } }';
    document.head.appendChild(style);
  }
}

window.initComments = initComments;

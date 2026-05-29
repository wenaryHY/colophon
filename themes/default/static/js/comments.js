(function () {
  function getPageLang() {
    var langAttr = (document.documentElement.lang || '');
    return langAttr.indexOf('zh') === 0 ? 'zh' : 'en';
  }

  var MSG = {
    zh: {
      commentSubmitted: '评论已提交审核。',
      submitError: '无法提交评论。',
      newCommentAppeared: '有一条新评论已发布。',
    },
    en: {
      commentSubmitted: 'Comment submitted for review.',
      submitError: 'Unable to submit comment.',
      newCommentAppeared: 'A new approved comment just appeared.',
    },
  };

  function t(key) {
    var lang = getPageLang();
    return (MSG[lang] && MSG[lang][key]) || MSG.en[key] || key;
  }

  function logDebug(event, details) {
    console.debug('[InkForge][comments][debug]', event, details || {});
  }

  function logError(event, details) {
    console.error('[InkForge][comments][error]', event, details || {});
  }

  function showToast(message, type) {
    const toastType = type || 'info';
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
    const colors = { success: '#10b981', error: '#ef4444', info: '#3b82f6' };
    Object.assign(toast.style, {
      background: colors[toastType] || colors.info,
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

  function escapeHtml(value) {
    return String(value || '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  async function submitComment(event) {
    event.preventDefault();
    const form = event.target;
    const data = new FormData(form);
    const slug = window.__POST_DATA__?.slug || '';
    logDebug('submit_start', { slug });

    try {
      await window.InkForgeApi.apiRequest(`/api/v1/posts/${slug}/comments`, {
        method: 'POST',
        body: {
          content: String(data.get('content') || '').trim(),
        },
      });

      showToast(t('commentSubmitted'), 'success');
      logDebug('submit_success', { slug });
      form.reset();
    } catch (error) {
      if (error && error.status === 401) {
        const target = window.InkForgeApi.sanitizeRedirect(window.location.pathname + window.location.search, '/');
        logDebug('submit_redirect_login', { slug, target });
        window.location.href = `/login?redirect=${encodeURIComponent(target)}`;
        return;
      }

      logError('submit_error', {
        slug,
        message: error.message,
        status: error.status,
      });
      showToast(error.message || t('submitError'), 'error');
    }
  }

  function initCommentWebSocket() {
    const postId = window.__POST_DATA__?.id;
    if (!postId) return;

    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    let reconnectDelay = 1000;

    function connect() {
      logDebug('ws_connect_attempt', { postId });
      const ws = new WebSocket(`${protocol}//${location.host}/ws/public?post_id=${encodeURIComponent(postId)}`);

      ws.onopen = function () {
        logDebug('ws_open', { postId });
        reconnectDelay = 1000;
      };

      ws.onmessage = function (messageEvent) {
        try {
          const event = JSON.parse(messageEvent.data);
          if (event.type === 'comment_approved' && event.data && event.data.post_id === postId) {
            const list = document.getElementById('comment-list');
            const countEl = document.getElementById('comment-count');

            if (list) {
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
        } catch (parseError) {
          logError('ws_parse_error', { postId, error: parseError });
        }
      };

      ws.onclose = function () {
        logDebug('ws_close', { postId, reconnectDelay });
        setTimeout(connect, reconnectDelay);
        reconnectDelay = Math.min(reconnectDelay * 2, 30000);
      };

      ws.onerror = function () {
        logError('ws_error', { postId });
        return undefined;
      };
    }

    connect();
  }

  function initComments() {
    const form = document.querySelector('.comment-form');
    logDebug('init', { hasForm: Boolean(form), postId: window.__POST_DATA__?.id || null });
    if (form) {
      form.addEventListener('submit', submitComment);
    }

    initCommentWebSocket();

    if (!document.getElementById('inkforge-fadein-style')) {
      const style = document.createElement('style');
      style.id = 'inkforge-fadein-style';
      style.textContent = '@keyframes fadeIn { from { opacity:0; transform:translateY(10px); } to { opacity:1; transform:translateY(0); } }';
      document.head.appendChild(style);
    }
  }

  window.initComments = initComments;
})();

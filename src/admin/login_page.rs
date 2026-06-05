use axum::response::{Html, IntoResponse};

/// 服务端渲染的登录页。
///
/// ## 为什么不用 SPA
/// 管理后台 SPA 有 `basename="/admin"`，所有 JS/CSS 硬编码 `/admin/assets/...` 路径，
/// 无法从 `/login` 提供服务。因此 `/admin` 未认证时重定向到本页（独立 HTML，无 SPA 依赖）。
///
/// ## 维护注意
/// 本页与 SPA 的 `Login.tsx` 共享以下逻辑，任一改动需同步：
/// - 登录 API 端点、请求/响应格式（`/api/v1/auth/login`）
/// - Turnstile site key 和主题
/// - 错误消息文案
///
/// ## 结构
/// - CSS (1-200行): 暗色 MD3 主题，CSS 变量与 SPA 保持一致
/// - HTML (201-270行): 居中卡片，用户名+密码+Turnstile+提交按钮
/// - JS (271-415行): Turnstile 初始化、表单提交、fetch API、错误处理
pub async fn serve_login_page() -> impl IntoResponse {
    Html(LOGIN_PAGE_HTML)
}

const LOGIN_PAGE_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8" />
<meta name="viewport" content="width=device-width, initial-scale=1.0" />
<title>InkForge — 登录</title>
<link rel="icon" type="image/svg+xml" href="/static/themes/default/logo-icon.svg">
<link rel="apple-touch-icon" href="/static/themes/default/logo-icon.svg">
<style>
  *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }

  <!-- CSS 变量 — 与 SPA MD3 主题保持一致，改动需同步 -->

  :root {
    --md-primary:              #f97316;
    --md-primary-dim:          #ea580c;
    --md-on-primary:           #ffffff;
    --md-primary-container:    #7c2d12;
    --md-on-primary-container: #fed7aa;

    --md-secondary:            #7f8c9a;
    --md-secondary-dim:        #64707d;
    --md-on-secondary:         #ffffff;
    --md-secondary-container:  #39424e;
    --md-on-secondary-container: #d8e3f8;

    --md-error:                #f87171;
    --md-error-dim:            #b91c1c;
    --md-on-error:             #450a0a;
    --md-error-container:      #7f1d1d;
    --md-on-error-container:   #fecaca;

    --md-background:               #111318;
    --md-on-background:            #e2e3e8;
    --md-surface:                  #111318;
    --md-on-surface:               #e2e3e8;
    --md-surface-variant:          #2e3039;
    --md-on-surface-variant:       #c4c6d0;
    --md-surface-container:        #1e2027;
    --md-surface-container-high:   #282a32;
    --md-surface-container-highest:#33353e;

    --md-outline:         #8b8d98;
    --md-outline-variant: #44464f;

    --font-family: "Inter", "Noto Sans SC", -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  body {
    font-family: var(--font-family);
    background: var(--md-background);
    color: var(--md-on-surface);
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100vh;
    padding: 1.5rem;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  @import url('https://fonts.loli.net/css2?family=Inter:wght@400;500;600;700&family=Noto+Sans+SC:wght@400;500;600;700&display=swap');

  .login-card {
    width: 100%;
    max-width: 26rem;
    background: var(--md-surface-container);
    border-radius: 1.5rem;
    padding: 2.5rem 2rem;
    border: 1px solid var(--md-outline-variant);
  }

  .login-header {
    text-align: center;
    margin-bottom: 2rem;
  }

  .login-title {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--md-on-surface);
    margin-bottom: 0.375rem;
    letter-spacing: -0.01em;
  }

  .login-subtitle {
    font-size: 0.875rem;
    color: var(--md-on-surface-variant);
    line-height: 1.5;
  }

  .form-group {
    margin-bottom: 1.25rem;
  }

  .form-label {
    display: block;
    font-size: 0.8125rem;
    font-weight: 500;
    color: var(--md-on-surface-variant);
    margin-bottom: 0.375rem;
    letter-spacing: 0.01em;
  }

  .form-input {
    width: 100%;
    padding: 0.625rem 0.875rem;
    border: 1px solid var(--md-outline-variant);
    border-radius: 0.625rem;
    background: var(--md-surface-container-highest);
    color: var(--md-on-surface);
    font-size: 0.9375rem;
    font-family: var(--font-family);
    line-height: 1.5;
    transition: border-color 0.15s ease, box-shadow 0.15s ease;
    outline: none;
  }

  .form-input:focus {
    border-color: var(--md-primary);
    box-shadow: 0 0 0 3px rgba(249, 115, 22, 0.15);
  }

  .form-input::placeholder {
    color: var(--md-outline);
  }

  .form-input:-webkit-autofill {
    -webkit-box-shadow: 0 0 0 1000px var(--md-surface-container-highest) inset;
    -webkit-text-fill-color: var(--md-on-surface);
  }

  .error-message {
    display: none;
    padding: 0.625rem 0.875rem;
    background: var(--md-error-container);
    color: var(--md-on-error-container);
    border-radius: 0.5rem;
    font-size: 0.8125rem;
    font-weight: 500;
    margin-bottom: 1rem;
    line-height: 1.4;
  }

  .error-message.visible {
    display: block;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    padding: 0.75rem 1.5rem;
    border: none;
    border-radius: 0.75rem;
    background: var(--md-primary);
    color: var(--md-on-primary);
    font-size: 0.9375rem;
    font-weight: 600;
    font-family: var(--font-family);
    cursor: pointer;
    transition: background 0.15s ease, box-shadow 0.15s ease, opacity 0.15s ease;
    letter-spacing: 0.01em;
  }

  .btn:hover {
    background: var(--md-primary-dim);
  }

  .btn:active {
    transform: scale(0.985);
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .turnstile-container {
    margin-bottom: 1.25rem;
    min-height: 65px;
    display: flex;
    justify-content: center;
  }

  .brand-footer {
    text-align: center;
    margin-top: 1.5rem;
    font-size: 0.75rem;
    color: var(--md-outline);
  }

  .spinner {
    display: inline-block;
    width: 1em;
    height: 1em;
    border: 2px solid transparent;
    border-top-color: currentColor;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
    margin-right: 0.5em;
    vertical-align: middle;
    box-sizing: content-box;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* Darken scrollbar for dark bg */
  ::-webkit-scrollbar { width: 6px; }
  ::-webkit-scrollbar-track { background: var(--md-background); }
  ::-webkit-scrollbar-thumb { background: var(--md-outline-variant); border-radius: 3px; }
  ::-webkit-scrollbar-thumb:hover { background: var(--md-outline); }
</style>
</head>

<!-- 页面结构 -->
<body>
<div class="login-card">
  <div class="login-header">
    <h1 class="login-title">InkForge</h1>
    <p class="login-subtitle">登录以访问管理后台</p>
  </div>

  <div id="login-error" class="error-message"></div>

  <form id="login-form" autocomplete="on" novalidate>
    <div class="form-group">
      <label class="form-label" for="username">用户名或邮箱</label>
      <input
        id="username"
        class="form-input"
        type="text"
        name="login"
        placeholder="请输入用户名或邮箱"
        autocomplete="username"
        autofocus
        required
      />
    </div>

    <div class="form-group">
      <label class="form-label" for="password">密码</label>
      <input
        id="password"
        class="form-input"
        type="password"
        name="password"
        placeholder="请输入密码"
        autocomplete="current-password"
        required
      />
    </div>

    <div id="turnstile-widget" class="turnstile-container"></div>

    <button id="submit-btn" class="btn" type="submit" disabled>
      登录
    </button>
  </form>

  <p class="brand-footer">InkForge &mdash; 你的幻想世界创作平台</p>
</div>

<script src="https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit" async defer></script>

<!-- 交互逻辑 — Turnstile + fetch 登录 -->
<script>
(function () {
  'use strict';

  var TURNSTILE_SITE_KEY = '0x4AAAAAADffbuvTrWkvKyda';
  var LOGIN_ENDPOINT = '/api/v1/auth/login';
  var ADMIN_URL = '/admin';

  var turnstileToken = '';
  var formEl = document.getElementById('login-form');
  var submitBtn = document.getElementById('submit-btn');
  var errorEl = document.getElementById('login-error');
  var turnstileContainer = document.getElementById('turnstile-widget');

  function showError(message) {
    errorEl.textContent = message;
    errorEl.classList.add('visible');
  }

  function hideError() {
    errorEl.textContent = '';
    errorEl.classList.remove('visible');
  }

  function setSubmitting(isSubmitting) {
    submitBtn.disabled = isSubmitting;
    if (isSubmitting) {
      submitBtn.innerHTML = '<span class="spinner"></span>登录中...';
    } else {
      submitBtn.textContent = '登录';
      submitBtn.disabled = !turnstileToken;
    }
  }

  // 加载 Turnstile widget
  function loadTurnstile() {
    if (typeof turnstile === 'undefined') {
      // Turnstile API 尚未加载，等待 DOM 加载后再试
      setTimeout(loadTurnstile, 200);
      return;
    }

    try {
      turnstile.render('#turnstile-widget', {
        sitekey: TURNSTILE_SITE_KEY,
        theme: 'dark',
        callback: function (token) {
          turnstileToken = token;
          submitBtn.disabled = false;
          hideError();
        },
        'expired-callback': function () {
          turnstileToken = '';
          submitBtn.disabled = true;
        },
        'error-callback': function () {
          turnstileToken = '';
          submitBtn.disabled = true;
          showError('人机验证加载失败，请刷新页面后重试');
        }
      });
    } catch (e) {
      showError('人机验证组件初始化失败，请检查网络后刷新页面');
      console.error('Turnstile render failed:', e);
    }
  }

  // Turnstile 用 onload 回调加载，listen for script load
  window.onloadTurnstileCallback = function () {
    loadTurnstile();
  };

  // 也处理 async 加载情况
  document.addEventListener('DOMContentLoaded', function () {
    loadTurnstile();
  });

  // 如果 DOMContentLoaded 已触发，直接调用
  if (document.readyState !== 'loading') {
    loadTurnstile();
  }

  formEl.addEventListener('submit', function (e) {
    e.preventDefault();
    hideError();

    var username = document.getElementById('username').value.trim();
    var password = document.getElementById('password').value;

    if (!username) {
      showError('请输入用户名或邮箱');
      return;
    }

    if (!password) {
      showError('请输入密码');
      return;
    }

    if (!turnstileToken) {
      showError('请完成人机验证');
      return;
    }

    setSubmitting(true);

    fetch(LOGIN_ENDPOINT, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        login: username,
        password: password,
        turnstile_token: turnstileToken,
        remember_me: false
      }),
      credentials: 'include'
    })
      .then(function (response) {
        if (!response.ok) {
          return response.json().then(function (body) {
            throw new Error(body.message || '登录失败，请检查用户名和密码');
          });
        }
        return response.json();
      })
      .then(function (data) {
        if (data.code === 0) {
          window.location.href = ADMIN_URL;
        } else {
          throw new Error(data.message || '登录失败');
        }
      })
      .catch(function (err) {
        // 解析可能的 JSON 错误消息
        var message = err.message || '网络错误，请检查连接后重试';
        showError(message);
        setSubmitting(false);

        // 重置 Turnstile（token 已失效）
        if (typeof turnstile !== 'undefined') {
          turnstile.reset();
        }
        turnstileToken = '';
      });
  });
})();
</script>
</body>
</html>"#;

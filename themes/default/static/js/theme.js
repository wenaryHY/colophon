/**
 * 暗色模式切换 — 在 <html> 上设置 data-theme 属性
 * 优先读 localStorage，其次系统偏好
 */
(function() {
  var html = document.documentElement;
  var saved = localStorage.getItem('colophon-theme');
  
  if (saved === 'dark') {
    html.setAttribute('data-theme', 'dark');
  } else if (saved === 'light') {
    html.setAttribute('data-theme', 'light');
  } else if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
    html.setAttribute('data-theme', 'dark');
  }

  // 初始化按钮文案
  var btns = document.querySelectorAll('[data-theme-toggle]');
  var current = html.getAttribute('data-theme');
  for (var i = 0; i < btns.length; i++) {
    btns[i].textContent = current === 'dark' ? '☀' : '☾';
  }
  
  // 监听系统偏好变化
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', function(e) {
    if (!localStorage.getItem('colophon-theme')) {
      html.setAttribute('data-theme', e.matches ? 'dark' : 'light');
    }
  });
})();

function toggleTheme() {
  var html = document.documentElement;
  var current = html.getAttribute('data-theme');
  var next = current === 'dark' ? 'light' : 'dark';
  html.setAttribute('data-theme', next);
  localStorage.setItem('colophon-theme', next);
  
  var btns = document.querySelectorAll('[data-theme-toggle]');
  for (var i = 0; i < btns.length; i++) {
    btns[i].textContent = next === 'dark' ? '☀' : '☾';
  }
}

/**
 * 语言切换器
 */
(function() {
  document.addEventListener('DOMContentLoaded', function() {
    var langButtons = document.querySelectorAll('.lang-switch__btn');
    if (langButtons.length === 0) return;
    
    // 从 localStorage 读取保存的语言偏好
    var savedLang = localStorage.getItem('preferred_language') || 'zh';
    
    // 设置初始激活状态
    for (var i = 0; i < langButtons.length; i++) {
      var btn = langButtons[i];
      var lang = btn.getAttribute('data-lang');
      
      if (lang === savedLang) {
        btn.classList.add('active');
        btn.setAttribute('aria-current', 'true');
      } else {
        btn.classList.remove('active');
        btn.removeAttribute('aria-current');
      }
      
      // 绑定点击事件
      btn.addEventListener('click', function() {
        var newLang = this.getAttribute('data-lang');
        
        // 保存到 localStorage
        localStorage.setItem('preferred_language', newLang);
        
        // 更新按钮状态
        for (var j = 0; j < langButtons.length; j++) {
          var b = langButtons[j];
          if (b.getAttribute('data-lang') === newLang) {
            b.classList.add('active');
            b.setAttribute('aria-current', 'true');
          } else {
            b.classList.remove('active');
            b.removeAttribute('aria-current');
          }
        }
        
        // 调用 i18n.js 切换文案（如果存在）
        if (window.I18n && window.I18n.init) {
          window.I18n.init(newLang);
        }
        
        // 如果已登录，调用 API 持久化
        if (window.ColophonApi) {
          window.ColophonApi.apiRequest('/api/v1/me/profile', {
            method: 'PATCH',
            body: { language: newLang }
          }).catch(function(err) {
            console.error('Failed to save language preference:', err);
          });
        }
      });
    }
  });
})();

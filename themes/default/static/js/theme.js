/**
 * 暗色模式切换 — 在 <html> 上设置 data-theme 属性
 * 优先读 localStorage，其次系统偏好
 */
(function() {
  var html = document.documentElement;
  var saved = localStorage.getItem('inkforge-theme');
  
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
    if (!localStorage.getItem('inkforge-theme')) {
      html.setAttribute('data-theme', e.matches ? 'dark' : 'light');
    }
  });
})();

function toggleTheme() {
  var html = document.documentElement;
  var current = html.getAttribute('data-theme');
  var next = current === 'dark' ? 'light' : 'dark';
  html.setAttribute('data-theme', next);
  localStorage.setItem('inkforge-theme', next);
  
  var btns = document.querySelectorAll('[data-theme-toggle]');
  for (var i = 0; i < btns.length; i++) {
    btns[i].textContent = next === 'dark' ? '☀' : '☾';
  }
}

// 前台国际化支持
(function() {
  'use strict';

  // 翻译字典
  const dictionary = {
    zh: {
      // 导航
      navHome: '首页',
      navArticles: '文章',
      navAbout: '关于',
      
      // 按钮
      buttonLogin: '登录',
      buttonProfile: '个人中心',
      
      // 页脚
      footerPrivacy: '隐私政策',
      footerTerms: '服务条款',
      
      // 评论
      commentTitle: '评论',
      commentSubmit: '发表评论',
      
      // 其他
      backToHome: '返回首页',
      
      // 登录页面
      loginTitle: '欢迎回来',
      loginSubtitle: '登录后即可评论、管理个人中心与同步会话。',
      loginLabel: '登录名',
      loginPlaceholder: '用户名或邮箱',
      passwordLabel: '密码',
      passwordPlaceholder: '请输入密码',
      loginButton: '登录',
      createAccount: '创建账号',
      backToHome: '返回首页',
      loginError: '登录失败，请稍后重试。',

      // 导航
      backHome: '返回首页',
      signOut: '退出登录',
      admin: '管理后台',

      // 用户
      role: '角色',
      joined: '加入于',

      // 设置
      profileSettings: '个人设置',
      displayName: '显示名称',
      language: '语言',
      themePreference: '主题偏好',
      system: '跟随系统',
      light: '浅色模式',
      dark: '深色模式',
      bio: '个人简介',
      bioPlaceholder: '写一段简短的介绍',
      saveProfile: '保存设置',

      // 密码
      changePassword: '修改密码',
      currentPassword: '当前密码',
      newPassword: '新密码',
      confirmPassword: '确认新密码',
      updatePassword: '更新密码',

      // 评论
      myComments: '我的评论',
      noComments: '暂无评论',
      loadCommentsError: '无法加载评论',

      // 统计
      posts: '文章',
      comments: '评论',
      likes: '点赞',

      // 提示
      profileUpdated: '个人资料已更新',
      passwordUpdated: '密码已更新',
      passwordMismatch: '两次输入的密码不一致',
    },
    en: {
      // Navigation
      navHome: 'Home',
      navArticles: 'Articles',
      navAbout: 'About',
      
      // Buttons
      buttonLogin: 'Login',
      buttonProfile: 'Profile',
      
      // Footer
      footerPrivacy: 'Privacy Policy',
      footerTerms: 'Terms of Service',
      
      // Comments
      commentTitle: 'Comments',
      commentSubmit: 'Submit Comment',
      
      // Other
      backToHome: 'Back to Home',
      
      // Login page
      loginTitle: 'Welcome back',
      loginSubtitle: 'Sign in to comment, manage your profile, and sync sessions.',
      loginLabel: 'Username',
      loginPlaceholder: 'Username or email',
      passwordLabel: 'Password',
      passwordPlaceholder: 'Enter your password',
      loginButton: 'Sign in',
      createAccount: 'Create account',
      backToHome: 'Back to home',
      loginError: 'Login failed. Please try again later.',

      // Nav
      backHome: 'Back home',
      signOut: 'Sign out',
      admin: 'Admin',

      // User
      role: 'role',
      joined: 'joined',

      // Settings
      profileSettings: 'Profile settings',
      displayName: 'Display name',
      language: 'Language',
      themePreference: 'Theme preference',
      system: 'System',
      light: 'Light',
      dark: 'Dark',
      bio: 'Bio',
      bioPlaceholder: 'Write a short bio',
      saveProfile: 'Save profile',

      // Password
      changePassword: 'Change password',
      currentPassword: 'Current password',
      newPassword: 'New password',
      confirmPassword: 'Confirm new password',
      updatePassword: 'Update password',

      // Comments
      myComments: 'My comments',
      noComments: 'No comments yet',
      loadCommentsError: 'Unable to load comments',

      // Stats
      posts: 'Posts',
      comments: 'Comments',
      likes: 'Likes',

      // Messages
      profileUpdated: 'Profile updated',
      passwordUpdated: 'Password updated',
      passwordMismatch: 'Password confirmation does not match',
    },
  };

  let currentLang = 'zh';

  // Cookie 工具函数
  function setCookie(name, value, days = 365) {
    const expires = new Date();
    expires.setTime(expires.getTime() + days * 24 * 60 * 60 * 1000);
    document.cookie = `${name}=${value};expires=${expires.toUTCString()};path=/`;
  }

  function getCookie(name) {
    const nameEQ = name + '=';
    const ca = document.cookie.split(';');
    for (let i = 0; i < ca.length; i++) {
      let c = ca[i];
      while (c.charAt(0) === ' ') c = c.substring(1, c.length);
      if (c.indexOf(nameEQ) === 0) return c.substring(nameEQ.length, c.length);
    }
    return null;
  }

  // 翻译函数
  function t(key, fallback) {
    const dict = dictionary[currentLang] || dictionary.zh;
    return dict[key] || fallback || key;
  }

  // 更新页面文本
  function updatePage() {
    // 更新所有带有 data-i18n 属性的元素
    document.querySelectorAll('[data-i18n]').forEach(el => {
      const key = el.getAttribute('data-i18n');
      if (key) {
        el.textContent = t(key, el.textContent);
      }
    });

    // 更新 placeholder
    document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
      const key = el.getAttribute('data-i18n-placeholder');
      if (key) {
        el.placeholder = t(key, el.placeholder);
      }
    });

    // 更新语言按钮状态
    document.querySelectorAll('.lang-switch__btn').forEach(btn => {
      const btnLang = btn.dataset.lang;
      if (btnLang === currentLang) {
        btn.classList.add('is-active');
      } else {
        btn.classList.remove('is-active');
      }
    });
  }

  // 初始化
  function init(lang) {
    if (lang) {
      currentLang = lang;
      setCookie('lang', lang);
    } else {
      // 尝试从 cookie 读取
      const savedLang = getCookie('lang');
      if (savedLang && (savedLang === 'zh' || savedLang === 'en')) {
        currentLang = savedLang;
      } else {
        // 使用浏览器语言
        const browserLang = navigator.language || navigator.userLanguage;
        currentLang = browserLang.startsWith('zh') ? 'zh' : 'en';
        setCookie('lang', currentLang);
      }
    }
    updatePage();
  }

  // 暴露到全局
  window.I18n = {
    t,
    init,
    updatePage,
    getLang: () => currentLang,
  };
})();

// 设置页面字典
export const settingsDict = {
  zh: {
    // 页面标题
    title: '站点设置',
    subtitle: '管理博客的基础配置、评论规则和主题外观',

    // 基础信息
    basicInfo: '基础信息',
    basicInfoDesc: '站点名称、描述等核心信息',
    siteTitle: '站点标题',
    siteDescription: '站点描述',
    siteUrl: '站点 URL',
    siteDescHint: '用于 SEO 和页面 meta 描述，建议不超过 160 字符',
    siteTimezone: '站点时区',
    siteTimezoneHint: 'IANA 时区标识符，如 UTC、Asia/Shanghai、America/New_York',
    siteUrlHint: '博客的完整访问地址，包含协议前缀',

    // 评论与注册
    commentsAndReg: '评论与注册',
    commentsAndRegDesc: '控制用户交互和内容审核策略',
    allowRegister: '公开注册',
    allowComment: '允许评论',
    commentRequireLogin: '评论需登录',
    moderationMode: '审核策略',
    commentMaxLength: '评论最大长度',
    allowRegisterOption1: '允许新用户注册',
    allowRegisterOption2: '关闭注册',
    allowCommentOption1: '允许评论',
    allowCommentOption2: '全局关闭评论',
    requireLoginOption1: '是 — 仅登录可评论',
    requireLoginOption2: '否 — 游客也可评论',
    moderationAll: '全部待审',
    moderationFirst: '首条待审，后续放行',
    moderationNone: '无需审核，直接发布',
    maxLengthHint: '单条评论允许的最大字符数',

    // 主题与外观
    themeAppearance: '主题与外观',
    themeAppearanceDesc: '切换和管理已安装的前台主题',
    currentTheme: '当前主题',
    defaultMode: '默认模式',
    modeSystem: '跟随系统',
    modeLight: '浅色模式',
    modeDark: '深色模式',
    installedThemes: '已安装的主题',
    themeActive: '使用中',

    // 回收站
    trashCleanup: '回收站与清理',
    trashCleanupDesc: '配置已删除内容的保留天数与自动清理时间',
    retentionDays: '保留天数',
    retentionDaysHint: '软删除的内容将被保存的天数，最长90天。过期后自动永久清理。',
    cleanupTime: '自动清理时间',
    cleanupTimeHint: '每天执行自动永久清理任务的时间。建议设在凌晨避开访问高峰。',

    // 界面设置
    uiSettings: '界面设置',
    uiSettingsDesc: '管理后台界面语言设置',
    interfaceLanguage: '界面语言',
    languageZh: '简体中文',
    languageEn: 'English',

    // 保存
    saveChanges: '保存更改',

    settingsSaved: '设置已保存',
    settingsSaveFailed: '保存设置失败',
    languageSaveFailed: '保存语言设置失败',
    // URL hint (extended)
    siteUrlHintFull: '博客的完整访问地址，必须是纯 origin，例如 https://example.com',
    adminUrlLabel: '后台 URL',
    adminUrlHint: 'admin_url 由 site_url 自动推导，修改 site_url 即可同步更新',
  },
  en: {
    // Page title
    title: 'Site Settings',
    subtitle: 'Manage basic configuration, comment rules and theme appearance',

    // Basic info
    basicInfo: 'Basic Information',
    basicInfoDesc: 'Site name, description and other core settings',
    siteTitle: 'Site Title',
    siteDescription: 'Site Description',
    siteUrl: 'Site URL',
    siteDescHint: 'Used for SEO and meta description, recommended max 160 characters',
    siteTimezone: 'Site Timezone',
    siteTimezoneHint: 'IANA timezone identifier, e.g. UTC, Asia/Shanghai, America/New_York',
    siteUrlHint: 'Full blog URL including protocol',

    // Comments & Registration
    commentsAndReg: 'Comments & Registration',
    commentsAndRegDesc: 'Control user interactions and content moderation',
    allowRegister: 'Public Registration',
    allowComment: 'Allow Comments',
    commentRequireLogin: 'Comment Requires Login',
    moderationMode: 'Moderation Policy',
    commentMaxLength: 'Max Comment Length',
    allowRegisterOption1: 'Allow new user registration',
    allowRegisterOption2: 'Disable registration',
    allowCommentOption1: 'Allow comments',
    allowCommentOption2: 'Disable comments globally',
    requireLoginOption1: 'Yes - Login required to comment',
    requireLoginOption2: 'No - Guests can comment',
    moderationAll: 'All pending review',
    moderationFirst: 'First pending, subsequent approved',
    moderationNone: 'No review required',
    maxLengthHint: 'Maximum characters allowed per comment',

    // Theme & Appearance
    themeAppearance: 'Theme & Appearance',
    themeAppearanceDesc: 'Switch and manage installed frontend themes',
    currentTheme: 'Current Theme',
    defaultMode: 'Default Mode',
    modeSystem: 'Follow System',
    modeLight: 'Light Mode',
    modeDark: 'Dark Mode',
    installedThemes: 'Installed Themes',
    themeActive: 'Active',

    // Trash
    trashCleanup: 'Trash & Cleanup',
    trashCleanupDesc: 'Configure retention days and auto-cleanup time',
    retentionDays: 'Retention Days',
    retentionDaysHint: 'Days to keep soft-deleted content, max 90. Expired items are permanently removed.',
    cleanupTime: 'Auto Cleanup Time',
    cleanupTimeHint: 'Daily cleanup schedule. Recommend off-peak hours.',

    // UI Settings
    uiSettings: 'Interface Settings',
    uiSettingsDesc: 'Admin panel language settings',
    interfaceLanguage: 'Interface Language',
    languageZh: '简体中文',
    languageEn: 'English',

    // Save
    saveChanges: 'Save Changes',

    settingsSaved: 'Settings saved',
    settingsSaveFailed: 'Failed to save settings',
    languageSaveFailed: 'Failed to save language setting',
    // URL hint (extended)
    siteUrlHintFull: 'Full blog URL, must be a pure origin, e.g. https://example.com',
    adminUrlLabel: 'Admin URL',
    adminUrlHint: 'admin_url is auto-derived from site_url. Update site_url to sync.',
  },
};

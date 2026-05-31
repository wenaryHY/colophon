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

    // 数据备份
    dataBackup: '数据备份',
    dataBackupDesc: '管理数据库备份，支持创建、下载、合并恢复和导入操作',
    createBackup: '创建备份',
    importBackup: '导入备份文件',
    backupHistory: '备份历史',
    noBackup: '暂无备份记录，点击上方「创建备份」生成第一份',
    backupMergeRestore: '合并恢复：保留当前新数据，合并此备份的历史数据',
    backupConfirm: '即将用 "{filename}" 替换当前数据库，原数据库会备份为 .bak 文件。是否继续？',

    // 界面设置
    uiSettings: '界面设置',
    uiSettingsDesc: '管理后台界面语言设置',
    interfaceLanguage: '界面语言',
    languageZh: '简体中文',
    languageEn: 'English',

    // 保存
    saveChanges: '保存更改',

    // 备份操作 toast
    creating: '创建中…',
    backupCreated: '已创建新备份',
    createBackupFailed: '创建备份失败',
    downloadBackupFailed: '下载备份失败',
    backupDownloadStarted: '备份文件已开始下载',
    mergeRestoreSuccess: '合并恢复成功，页面即将刷新',
    mergeRestoreFailed: '合并恢复失败',
    mergeRestoreConfirm: '将执行"合并恢复"：保留当前新数据并合并备份历史数据，是否继续？',
    backupDeleted: '备份已删除',
    deleteBackupFailed: '删除备份失败',
    deleteBackupConfirm: '确定删除这个备份吗？删除后不可恢复。',
    backupImportSuccess: '备份导入成功，页面将刷新...',
    importFailed: '导入失败',
    settingsSaved: '设置已保存',
    settingsSaveFailed: '保存设置失败',
    languageSaveFailed: '保存语言设置失败',
    backupHistoryCount: '备份历史 ({count})',

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

    // Data Backup
    dataBackup: 'Data Backup',
    dataBackupDesc: 'Manage database backups with create, download, merge-restore and import',
    createBackup: 'Create Backup',
    importBackup: 'Import Backup',
    backupHistory: 'Backup History',
    noBackup: 'No backups yet. Click "Create Backup" to generate the first one.',
    backupMergeRestore: 'Merge restore: keep current data and merge historical data from backup',
    backupConfirm: 'About to replace current database with "{filename}". Original will be saved as .bak. Continue?',

    // UI Settings
    uiSettings: 'Interface Settings',
    uiSettingsDesc: 'Admin panel language settings',
    interfaceLanguage: 'Interface Language',
    languageZh: '简体中文',
    languageEn: 'English',

    // Save
    saveChanges: 'Save Changes',

    // Backup toast messages
    creating: 'Creating...',
    backupCreated: 'Backup created',
    createBackupFailed: 'Failed to create backup',
    downloadBackupFailed: 'Failed to download backup',
    backupDownloadStarted: 'Backup download started',
    mergeRestoreSuccess: 'Merge restore successful, page will refresh',
    mergeRestoreFailed: 'Merge restore failed',
    mergeRestoreConfirm: 'About to perform "merge restore": keep current new data and merge historical data from backup. Continue?',
    backupDeleted: 'Backup deleted',
    deleteBackupFailed: 'Failed to delete backup',
    deleteBackupConfirm: 'Are you sure you want to delete this backup? This cannot be undone.',
    backupImportSuccess: 'Backup imported successfully, page will refresh...',
    importFailed: 'Import failed',
    settingsSaved: 'Settings saved',
    settingsSaveFailed: 'Failed to save settings',
    languageSaveFailed: 'Failed to save language setting',
    backupHistoryCount: 'Backup History ({count})',

    // URL hint (extended)
    siteUrlHintFull: 'Full blog URL, must be a pure origin, e.g. https://example.com',
    adminUrlLabel: 'Admin URL',
    adminUrlHint: 'admin_url is auto-derived from site_url. Update site_url to sync.',
  },
};

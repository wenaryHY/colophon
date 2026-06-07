// 备份页面字典
export const backupDict = {
  zh: {
    // 页面标题
    title: '备份管理',
    subtitle: '管理数据库定时备份、手动操作和备份历史',

    // 定时备份
    scheduledBackup: '定时备份',
    scheduledBackupDesc: '配置自动备份计划，系统将按设定周期自动创建备份',
    enableScheduledBackup: '启用定时备份',
    backupFrequency: '备份频率',
    backupTime: '备份时间',
    backupStorage: '存储方式',
    saveSchedule: '保存计划',
    scheduleSaved: '定时备份计划已保存',
    scheduleSaveFailed: '保存计划失败',
    scheduleNextRun: '下次执行',
    scheduleLastRun: '上次执行',
    scheduleNever: '尚未执行',
    frequencyDaily: '每天',
    frequencyHourly: '每小时',
    providerLocal: '本地存储',
    providerS3: 'S3 兼容',

    // 手动操作
    manualBackup: '手动操作',
    manualBackupDesc: '随时创建新备份或从文件导入备份',
    createBackup: '创建备份',
    importBackup: '导入备份',

    // 备份历史
    backupHistory: '备份历史',
    backupHistoryCount: '备份历史 ({count})',
    noBackup: '暂无备份记录，点击上方「创建备份」生成第一份',
    backupMergeRestore: '合并恢复：保留当前新数据，合并此备份的历史数据',
    backupConfirm: '即将用 "{filename}" 替换当前数据库，原数据库会备份为 .bak 文件。是否继续？',

    // Toast / 操作提示
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
  },
  en: {
    // Page title
    title: 'Backups',
    subtitle: 'Manage scheduled backups, manual operations and backup history',

    // Scheduled backup
    scheduledBackup: 'Scheduled Backup',
    scheduledBackupDesc: 'Configure automatic backup schedule. System will create backups periodically.',
    enableScheduledBackup: 'Enable Scheduled Backup',
    backupFrequency: 'Backup Frequency',
    backupTime: 'Backup Time',
    backupStorage: 'Storage Provider',
    saveSchedule: 'Save Schedule',
    scheduleSaved: 'Backup schedule saved',
    scheduleSaveFailed: 'Failed to save schedule',
    scheduleNextRun: 'Next Run',
    scheduleLastRun: 'Last Run',
    scheduleNever: 'Never',
    frequencyDaily: 'Daily',
    frequencyHourly: 'Hourly',
    providerLocal: 'Local Storage',
    providerS3: 'S3 Compatible',

    // Manual backup
    manualBackup: 'Manual Operations',
    manualBackupDesc: 'Create new backups or import from file at any time',
    createBackup: 'Create Backup',
    importBackup: 'Import Backup',

    // Backup history
    backupHistory: 'Backup History',
    backupHistoryCount: 'Backup History ({count})',
    noBackup: 'No backup records yet. Click "Create Backup" above to generate the first one.',
    backupMergeRestore: 'Merge restore: keep current data and merge historical data from backup',
    backupConfirm: 'About to replace current database with "{filename}". Original will be saved as .bak. Continue?',

    // Toast / actions
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
  },
};

/**
 * FAB 配置共享模块
 * 提供配置类型定义及 localStorage 读写，供 FabContainer 和 FabSettings 共同使用
 */

// ==================== 常量 ====================

/** FAB 配置 localStorage 键 */
export const FAB_CONFIG_STORAGE_KEY = 'fab-config';

// ==================== 类型定义 ====================

/** FAB 操作项配置 */
export interface FabAction {
  /** 唯一标识 */
  id: string;
  /** 操作类型：内置预览 or 自定义 */
  type: 'preview' | 'custom';
  /** 图标（Emoji 字符串） */
  icon: string;
  /** 操作名称 */
  label: string;
  /** 是否启用 */
  enabled: boolean;
  /** 排序序号（越小越靠前） */
  order: number;
  /** 自定义操作的执行配置 */
  action?: {
    /** 执行类型：打开 URL / 运行脚本 / 触发事件 */
    type: 'url' | 'script' | 'event';
    /** 执行值（URL 地址 / 脚本内容 / 事件名称） */
    value: string;
  };
}

/** FAB 全局配置 */
export interface FabConfig {
  /** 是否启用 FAB */
  enabled: boolean;
  /** 是否可拖拽 */
  draggable: boolean;
  /** 自动吸附边缘 */
  snapToEdge: boolean;
  /** 滚动时自动隐藏 */
  autoHide: boolean;
  /** 操作项列表 */
  actions: FabAction[];
}

// ==================== 默认配置 ====================

export const DEFAULT_FAB_CONFIG: FabConfig = {
  enabled: true,
  draggable: true,
  snapToEdge: true,
  autoHide: false,
  actions: [
    {
      id: 'preview',
      type: 'preview',
      icon: '👁',
      label: '实时预览',
      enabled: true,
      order: 0,
    },
  ],
};

// ==================== 工具函数 ====================

/**
 * 从 localStorage 加载 FAB 配置
 * 失败或缺失时返回默认配置，并与默认值合并以兼容旧版本
 */
export function loadFabConfig(): FabConfig {
  try {
    const raw = localStorage.getItem(FAB_CONFIG_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<FabConfig>;
      return {
        ...DEFAULT_FAB_CONFIG,
        ...parsed,
        actions: parsed.actions ?? DEFAULT_FAB_CONFIG.actions,
      };
    }
  } catch {
    // localStorage 不可用或数据损坏时忽略
  }
  return DEFAULT_FAB_CONFIG;
}

/**
 * 将 FAB 配置持久化到 localStorage
 */
export function saveFabConfig(config: FabConfig): void {
  try {
    localStorage.setItem(FAB_CONFIG_STORAGE_KEY, JSON.stringify(config));
  } catch {
    // 存储满或隐私模式下忽略
  }
}

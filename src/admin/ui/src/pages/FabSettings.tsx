/**
 * FabSettings — FAB（浮动操作按钮）配置页面
 *
 * 功能：
 * - 基础设置：启用/禁用 FAB、可拖拽、自动吸附边缘、滚动时自动隐藏
 * - 操作项管理：查看、排序（上下箭头）、添加、编辑、删除自定义操作
 * - 数据持久化：localStorage 存储，支持恢复默认配置
 * - FAB 已关闭时可在此重新打开
 */
import { useState, useCallback, useMemo, useEffect } from 'react';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Select, SelectItem } from '../components/Select';
import { Modal } from '../components/Modal';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';
import {
  loadFabConfig,
  saveFabConfig,
  DEFAULT_FAB_CONFIG,
  type FabConfig,
  type FabAction,
} from '../fab/FabConfigStorage';

/** 生成唯一 ID */
function generateId(): string {
  return `action-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

// ==================== 内联样式 ====================

const sectionStyle: React.CSSProperties = {
  background: 'var(--md-surface-container-lowest)',
  borderRadius: 'var(--radius-lg)',
  marginBottom: '20px',
};
const secHeadStyle: React.CSSProperties = {
  padding: '18px 24px',
  background: 'var(--md-surface-container-low)',
};
const secTitleStyle: React.CSSProperties = {
  fontSize: '15px',
  fontWeight: 700,
  color: 'var(--md-on-surface)',
  letterSpacing: '-0.2px',
};
const secDescStyle: React.CSSProperties = {
  fontSize: '12.5px',
  color: 'var(--md-outline)',
  marginTop: '3px',
};
const secBodyStyle: React.CSSProperties = {
  padding: '24px',
  display: 'flex',
  flexDirection: 'column',
  gap: '18px',
};

const toggleRowStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  padding: '8px 0',
};
const toggleLabelGroupStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '2px',
};
const toggleLabelStyle: React.CSSProperties = {
  fontSize: '14px',
  fontWeight: 600,
  color: 'var(--md-on-surface)',
};
const toggleDescStyle: React.CSSProperties = {
  fontSize: '12.5px',
  color: 'var(--md-outline)',
};

/** Toggle 开关 — MD3 Switch 风格 */
function Toggle({
  checked,
  onChange,
  disabled = false,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => !disabled && onChange(!checked)}
      style={{
        position: 'relative',
        width: '52px',
        height: '32px',
        borderRadius: '16px',
        border: 'none',
        cursor: disabled ? 'not-allowed' : 'pointer',
        background: checked ? 'var(--md-primary)' : 'var(--md-surface-container-highest)',
        outline: 'none',
        transition: 'background 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
        flexShrink: 0,
        opacity: disabled ? 0.5 : 1,
      }}
      onMouseEnter={(e) => {
        if (!disabled) {
          e.currentTarget.style.boxShadow = '0 0 0 4px rgba(249,115,22,0.12)';
        }
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.boxShadow = 'none';
      }}
    >
      {/* 滑块 */}
      <div
        style={{
          position: 'absolute',
          top: '4px',
          left: checked ? '24px' : '4px',
          width: '24px',
          height: '24px',
          borderRadius: '12px',
          background: checked ? 'var(--md-on-primary)' : 'var(--md-outline)',
          transition: 'left 0.2s cubic-bezier(0.4, 0, 0.2, 1), background 0.2s ease',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        {/* 选中态内圈 */}
        {checked && (
          <div
            style={{
              width: '12px',
              height: '12px',
              borderRadius: '6px',
              background: 'var(--md-primary)',
              transition: 'transform 0.15s ease',
            }}
          />
        )}
      </div>
    </button>
  );
}

/** 单个操作项行 */
function ActionItem({
  action,
  isOnlyItem,
  onMoveUp,
  onMoveDown,
  canMoveUp,
  canMoveDown,
  onEdit,
  onDelete,
}: {
  action: FabAction;
  isOnlyItem: boolean;
  canMoveUp: boolean;
  canMoveDown: boolean;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  const isPreview = action.type === 'preview';

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '12px',
        padding: '12px 16px',
        borderRadius: 'var(--radius-md)',
        background: 'var(--md-surface-container)',
        transition: 'background 0.15s ease',
      }}
      onMouseEnter={(e) => {
        e.currentTarget.style.background = 'var(--md-surface-container-high)';
      }}
      onMouseLeave={(e) => {
        e.currentTarget.style.background = 'var(--md-surface-container)';
      }}
    >
      {/* 排序按钮 */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', flexShrink: 0 }}>
        <button
          type="button"
          aria-label="上移"
          disabled={!canMoveUp}
          onClick={onMoveUp}
          style={{
            width: '24px',
            height: '20px',
            border: 'none',
            borderRadius: '4px',
            background: canMoveUp ? 'var(--md-surface-container-highest)' : 'transparent',
            color: canMoveUp ? 'var(--md-on-surface-variant)' : 'var(--md-outline)',
            cursor: canMoveUp ? 'pointer' : 'default',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: '11px',
            lineHeight: 1,
            transition: 'all 0.15s ease',
          }}
        >
          ▲
        </button>
        <button
          type="button"
          aria-label="下移"
          disabled={!canMoveDown}
          onClick={onMoveDown}
          style={{
            width: '24px',
            height: '20px',
            border: 'none',
            borderRadius: '4px',
            background: canMoveDown ? 'var(--md-surface-container-highest)' : 'transparent',
            color: canMoveDown ? 'var(--md-on-surface-variant)' : 'var(--md-outline)',
            cursor: canMoveDown ? 'pointer' : 'default',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: '11px',
            lineHeight: 1,
            transition: 'all 0.15s ease',
          }}
        >
          ▼
        </button>
      </div>

      {/* 图标 */}
      <span style={{ fontSize: '18px', flexShrink: 0 }}>{action.icon}</span>

      {/* 名称 + 类型标签 */}
      <div style={{ flex: 1, minWidth: 0 }}>
        <span style={{ fontSize: '14px', fontWeight: 600, color: 'var(--md-on-surface)' }}>
          {action.label}
        </span>
        {isPreview && (
          <span
            style={{
              marginLeft: '8px',
              fontSize: '10.5px',
              fontWeight: 600,
              padding: '1px 6px',
              borderRadius: '4px',
              background: 'rgba(249,115,22,0.1)',
              color: 'var(--md-primary)',
              verticalAlign: 'middle',
            }}
          >
            内置
          </span>
        )}
      </div>

      {/* 操作按钮 */}
      <div style={{ display: 'flex', gap: '6px', flexShrink: 0 }}>
        <Button variant="ghost" size="sm" onClick={onEdit}>
          编辑
        </Button>
        {!isPreview && !isOnlyItem && (
          <Button
            variant="ghost"
            size="sm"
            onClick={onDelete}
            style={{ color: '#ef4444' }}
          >
            删除
          </Button>
        )}
      </div>
    </div>
  );
}

// ==================== 主组件 ====================

export default function FabSettings() {
  const toast = useToast();
  const { t } = useI18n();

  // ---- 配置状态 ----
  const [config, setConfig] = useState<FabConfig>(() => loadFabConfig());

  // ---- Modal 状态 ----
  const [modalOpen, setModalOpen] = useState(false);
  const [editingAction, setEditingAction] = useState<FabAction | null>(null);

  // ---- 表单状态 ----
  const [formLabel, setFormLabel] = useState('');
  const [formIcon, setFormIcon] = useState('⚙️');
  const [formActionType, setFormActionType] = useState<'url' | 'script' | 'event'>('url');
  const [formActionValue, setFormActionValue] = useState('');
  const [formErrors, setFormErrors] = useState<Record<string, string>>({});

  const [fabHidden, setFabHidden] = useState(
    () => localStorage.getItem('fab-hidden') === 'true',
  );

  useEffect(() => {
    const handler = () =>
      setFabHidden(localStorage.getItem('fab-hidden') === 'true');
    window.addEventListener('fab-visibility-change', handler);
    return () => window.removeEventListener('fab-visibility-change', handler);
  }, []);

  // ---- 按 order 排序后的操作项 ----
  const sortedActions = useMemo(
    () => [...config.actions].sort((a, b) => a.order - b.order),
    [config.actions],
  );

  // ==================== 基础设置变更 ====================

  /** 更新布尔类型的全局配置项 */
  const toggleSetting = useCallback(
    (key: 'enabled' | 'draggable' | 'snapToEdge' | 'autoHide') => {
      setConfig((prev) => ({ ...prev, [key]: !prev[key] }));
    },
    [],
  );

  // ==================== 排序操作 ====================

  /** 交换两个操作项的 order 值 */
  const swapOrder = useCallback(
    (indexA: number, indexB: number) => {
      setConfig((prev) => {
        const next = [...prev.actions];
        const tempOrder = next[indexA].order;
        next[indexA] = { ...next[indexA], order: next[indexB].order };
        next[indexB] = { ...next[indexB], order: tempOrder };
        return { ...prev, actions: next };
      });
    },
    [],
  );

  // ==================== 编辑操作 ====================

  /** 打开编辑 Modal */
  const openEditModal = useCallback((action: FabAction) => {
    setEditingAction(action);
    setFormLabel(action.label);
    setFormIcon(action.icon);
    setFormActionType(action.action?.type ?? 'url');
    setFormActionValue(action.action?.value ?? '');
    setFormErrors({});
    setModalOpen(true);
  }, []);

  /** 打开新建 Modal */
  const openCreateModal = useCallback(() => {
    setEditingAction(null);
    setFormLabel('');
    setFormIcon('⚙️');
    setFormActionType('url');
    setFormActionValue('');
    setFormErrors({});
    setModalOpen(true);
  }, []);

  // ==================== 表单验证 ====================

  /** 校验表单字段，返回是否通过 */
  const validateForm = useCallback((): boolean => {
    const errors: Record<string, string> = {};
    if (!formLabel.trim()) {
      errors.label = '请输入操作名称';
    }
    if (!formIcon.trim()) {
      errors.icon = '请输入图标 Emoji';
    }
    if (!formActionValue.trim()) {
      errors.value = '请输入执行值';
    }
    setFormErrors(errors);
    return Object.keys(errors).length === 0;
  }, [formLabel, formIcon, formActionValue]);

  // ==================== 提交操作 ====================

  /** 保存新建/编辑的操作项 */
  const handleSaveAction = useCallback(() => {
    if (!validateForm()) return;

    setConfig((prev) => {
      const newAction: FabAction = {
        id: editingAction?.id ?? generateId(),
        type: editingAction?.type ?? 'custom',
        icon: formIcon.trim(),
        label: formLabel.trim(),
        enabled: editingAction?.enabled ?? true,
        order: editingAction?.order ?? prev.actions.length,
        action: {
          type: formActionType,
          value: formActionValue.trim(),
        },
      };

      let nextActions: FabAction[];
      if (editingAction) {
        // 编辑模式：替换对应项
        nextActions = prev.actions.map((a) => (a.id === editingAction.id ? newAction : a));
      } else {
        // 新建模式：追加到末尾
        nextActions = [...prev.actions, newAction];
      }
      return { ...prev, actions: nextActions };
    });

    setModalOpen(false);
    toast(editingAction ? '操作已更新' : '操作已添加', 'success');
  }, [editingAction, formLabel, formIcon, formActionType, formActionValue, validateForm, toast]);

  // ==================== 删除操作 ====================

  /** 删除指定操作项 */
  const handleDeleteAction = useCallback(
    (actionId: string) => {
      setConfig((prev) => ({
        ...prev,
        actions: prev.actions.filter((a) => a.id !== actionId),
      }));
      toast('操作已删除', 'success');
    },
    [toast],
  );

  // ==================== 保存与恢复 ====================

  /** 保存配置到 localStorage */
  const handleSave = useCallback(() => {
    saveFabConfig(config);
    toast('FAB 配置已保存', 'success');
  }, [config, toast]);

  /** 恢复默认配置 */
  const handleRestoreDefaults = useCallback(() => {
    setConfig(DEFAULT_FAB_CONFIG);
    saveFabConfig(DEFAULT_FAB_CONFIG);
    toast('已恢复默认配置', 'success');
  }, [toast]);

  /** 重新打开 FAB（清除隐藏标记） */
  const handleReopenFab = useCallback(() => {
    localStorage.removeItem('fab-hidden');
    window.dispatchEvent(new Event('fab-visibility-change'));
    toast('FAB 已重新打开', 'success');
  }, [toast]);

  // ==================== Modal 标题 ====================
  const modalTitle = editingAction ? '编辑操作' : '添加新操作';

  // ==================== 渲染 ====================

  return (
    <>
      <PageHeader
        title={t('fabSettingsTitle')}
        subtitle={t('fabSettingsSubtitle')}
        actions={
          <div style={{ display: 'flex', gap: '8px' }}>
            <Button variant="ghost" onClick={handleRestoreDefaults}>
              恢复默认
            </Button>
            <Button onClick={handleSave}>保存</Button>
          </div>
        }
      />

      {/* FAB 已关闭提示 */}
      {fabHidden && (
        <div style={sectionStyle}>
          <div style={{
            ...secBodyStyle,
            alignItems: 'center',
            textAlign: 'center',
            gap: '12px',
          }}>
            <p style={{
              fontSize: '14px',
              color: 'var(--md-on-surface-variant)',
              margin: 0,
            }}>
              {t('fabClosedHint')}
            </p>
            <Button onClick={handleReopenFab}>
              {t('fabEnable')}
            </Button>
          </div>
        </div>
      )}

      {/* ── 基础设置 ── */}
      <div style={sectionStyle}>
        <div style={secHeadStyle}>
          <h3 style={secTitleStyle}>{t('fabBasicSettings')}</h3>
          <p style={secDescStyle}>控制 FAB 的全局行为</p>
        </div>
        <div style={secBodyStyle}>
          <div style={toggleRowStyle}>
            <div style={toggleLabelGroupStyle}>
              <span style={toggleLabelStyle}>{t('fabEnable')}</span>
              <span style={toggleDescStyle}>启用后将在前台页面显示浮动操作按钮</span>
            </div>
            <Toggle checked={config.enabled} onChange={() => toggleSetting('enabled')} />
          </div>
          <div style={toggleRowStyle}>
            <div style={toggleLabelGroupStyle}>
              <span style={toggleLabelStyle}>{t('fabDraggable')}</span>
              <span style={toggleDescStyle}>允许用户拖拽 FAB 到页面任意位置</span>
            </div>
            <Toggle checked={config.draggable} onChange={() => toggleSetting('draggable')} />
          </div>
          <div style={toggleRowStyle}>
            <div style={toggleLabelGroupStyle}>
              <span style={toggleLabelStyle}>{t('fabSnapToEdge')}</span>
              <span style={toggleDescStyle}>松手后自动吸附到最近的屏幕边缘</span>
            </div>
            <Toggle checked={config.snapToEdge} onChange={() => toggleSetting('snapToEdge')} />
          </div>
          <div style={toggleRowStyle}>
            <div style={toggleLabelGroupStyle}>
              <span style={toggleLabelStyle}>{t('fabAutoHide')}</span>
              <span style={toggleDescStyle}>用户向下滚动时自动隐藏，向上滚动时重新显示</span>
            </div>
            <Toggle checked={config.autoHide} onChange={() => toggleSetting('autoHide')} />
          </div>
        </div>
      </div>

      {/* ── 操作项列表 ── */}
      <div style={sectionStyle}>
        <div style={secHeadStyle}>
          <h3 style={secTitleStyle}>{t('fabActions')}</h3>
          <p style={secDescStyle}>自定义 FAB 展开后的操作项，可拖拽排序</p>
        </div>
        <div style={secBodyStyle}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
            {sortedActions.map((action, index) => (
              <ActionItem
                key={action.id}
                action={action}
                isOnlyItem={sortedActions.length === 1}
                canMoveUp={index > 0}
                canMoveDown={index < sortedActions.length - 1}
                onMoveUp={() => swapOrder(index, index - 1)}
                onMoveDown={() => swapOrder(index, index + 1)}
                onEdit={() => openEditModal(action)}
                onDelete={() => handleDeleteAction(action.id)}
              />
            ))}
          </div>

          {/* 添加按钮 */}
          <Button variant="ghost" onClick={openCreateModal}>
            <svg
              width="14"
              height="14"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <path d="M12 5v14" />
              <path d="M5 12h14" />
            </svg>
            添加新操作
          </Button>
        </div>
      </div>

      {/* ── 新建/编辑操作 Modal ── */}
      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title={modalTitle}
        actions={
          <>
            <Button variant="ghost" onClick={() => setModalOpen(false)}>
              取消
            </Button>
            <Button onClick={handleSaveAction}>
              {editingAction ? '保存更改' : '添加'}
            </Button>
          </>
        }
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
          {/* 操作名称 */}
          <Input
            label={t('fabActionName')}
            value={formLabel}
            onChange={(e) => setFormLabel(e.target.value)}
            placeholder="例如：主题切换"
            error={formErrors.label}
          />

          {/* 图标 Emoji */}
          <Input
            label={t('fabActionIcon')}
            value={formIcon}
            onChange={(e) => setFormIcon(e.target.value)}
            placeholder="例如：⚙️"
            error={formErrors.icon}
          />

          {/* 操作类型 */}
          <Select
            label={t('fabActionType')}
            value={formActionType}
            onChange={(e) => setFormActionType(e.target.value as 'url' | 'script' | 'event')}
          >
            <SelectItem value="url">{t('fabActionTypeUrl')}</SelectItem>
            <SelectItem value="script">{t('fabActionTypeScript')}</SelectItem>
            <SelectItem value="event">{t('fabActionTypeEvent')}</SelectItem>
          </Select>

          {/* 执行值 */}
          <Input
            label={t('fabActionValue')}
            value={formActionValue}
            onChange={(e) => setFormActionValue(e.target.value)}
            placeholder={
              formActionType === 'url'
                ? 'https://example.com'
                : formActionType === 'script'
                  ? 'alert("Hello!")'
                  : 'toggle-theme'
            }
            error={formErrors.value}
          />
        </div>
      </Modal>
    </>
  );
}

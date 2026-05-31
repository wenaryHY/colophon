/**
 * FabContainer — 浮动操作按钮容器
 *
 * 功能：
 * - 主按钮点击展开/收起 Speed Dial 菜单
 * - 支持拖拽定位（桌面端）
 * - 子项依次弹出动画（间隔 100ms）
 * - 半透明遮罩层，点击关闭菜单
 * - 响应式：移动端底部居中禁止拖拽，桌面端右下角可拖拽
 * - 无障碍：aria-label、键盘导航、role 属性
 */
import { useState, useCallback, useEffect, useRef } from 'react';
import { useDraggable } from './useDraggable';
import { IconEye } from '../components/Icons';
import { PreviewRenderer } from '../preview/PreviewRenderer';

// ==================== 类型定义 ====================

/** 单个 FAB 操作项 */
export interface FabAction {
  /** 唯一标识 */
  id: string;
  /** 图标节点 */
  icon: React.ReactNode;
  /** 操作名称（用于无障碍标签） */
  label: string;
  /** 点击回调 */
  onClick: () => void;
  /** 自定义颜色（可选） */
  color?: string;
  /** 是否可见（默认 true） */
  visible?: boolean;
}

/** FabContainer 组件属性 */
export interface FabContainerProps {
  /** 操作项列表 */
  actions: FabAction[];
  /** 默认位置 */
  defaultPosition?: { x: number; y: number };
  /** 是否可拖拽（默认 true） */
  draggable?: boolean;
  /** 展开方向（默认 'up'） */
  expandDirection?: 'up' | 'down' | 'left' | 'right';
  /** 主按钮图标（默认 IconEye） */
  mainIcon?: React.ReactNode;
}

// ==================== 常量 ====================

/** 主按钮尺寸（px） */
const MAIN_BUTTON_SIZE = 56;

/** 子按钮尺寸（px） */
const ACTION_BUTTON_SIZE = 40;

/** 子项间距（px） */
const ACTION_GAP = 16;

/** 子项弹出间隔（ms） */
const ACTION_STAGGER_DELAY = 100;

/** 移动端断点（px） */
const MOBILE_BREAKPOINT = 768;

/** 惰性获取默认初始位置（右下角），避免模块顶层直接读取 window 尺寸导致闪现到 (0,0) */
function getDefaultPosition() {
  return { x: window.innerWidth - 80, y: window.innerHeight - 80 };
}

// ==================== 辅助函数 ====================

/**
 * 判断当前是否为移动端视图
 * @returns 是否小于移动端断点宽度
 */
function isMobileView(): boolean {
  return window.innerWidth < MOBILE_BREAKPOINT;
}

/**
 * 根据展开方向计算子项偏移量
 * @param direction - 展开方向
 * @param index - 子项索引（从 0 开始）
 * @returns { x, y } 偏移量
 */
function calculateActionOffset(
  direction: 'up' | 'down' | 'left' | 'right',
  index: number
): { x: number; y: number } {
  // 每个子项占据的空间 = 子按钮尺寸 + 间距
  const step = ACTION_BUTTON_SIZE + ACTION_GAP;
  const offset = (index + 1) * step;

  switch (direction) {
    case 'up':
      return { x: 0, y: -offset };
    case 'down':
      return { x: 0, y: offset };
    case 'left':
      return { x: -offset, y: 0 };
    case 'right':
      return { x: offset, y: 0 };
  }
}

// ==================== 子组件 ====================

/**
 * MainButton — 主按钮
 * 56px 圆形，作为拖拽手柄和展开触发器
 */
function MainButton({
  icon,
  isOpen,
  isDragging,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  dragRef,
  onClick,
}: {
  icon: React.ReactNode;
  isOpen: boolean;
  isDragging: boolean;
  onPointerDown: (e: React.PointerEvent) => void;
  onPointerMove: (e: React.PointerEvent) => void;
  onPointerUp: (e: React.PointerEvent) => void;
  dragRef: React.RefObject<HTMLDivElement | null>;
  onClick: () => void;
}) {
  /** 处理点击：拖拽中禁止触发点击 */
  const handleClick = useCallback(() => {
    if (!isDragging) {
      onClick();
    }
  }, [isDragging, onClick]);

  /** 处理键盘事件：Enter/Space 触发点击 */
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleClick();
      }
    },
    [handleClick]
  );

  return (
    <div
      ref={dragRef}
      role="button"
      tabIndex={0}
      aria-label={isOpen ? '关闭菜单' : '打开菜单'}
      aria-expanded={isOpen}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      style={{
        width: MAIN_BUTTON_SIZE,
        height: MAIN_BUTTON_SIZE,
        borderRadius: '50%',
        backgroundColor: 'var(--md-primary)',
        color: 'var(--md-on-primary)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: isDragging ? 'grabbing' : 'grab',
        boxShadow: isDragging
          ? '0 8px 24px rgba(249, 115, 22, 0.4)'
          : 'var(--elevation-3)',
        transform: isDragging ? 'scale(1.1)' : 'scale(1)',
        transition: 'box-shadow 0.2s, transform 0.2s',
        touchAction: 'none',
        userSelect: 'none',
        zIndex: 1001,
        outline: 'none',
      }}
    >
      {/* 旋转图标：展开时顺时针旋转 45 度形成 "+" 变形效果 */}
      <div
        style={{
          transform: isOpen ? 'rotate(45deg)' : 'rotate(0deg)',
          transition: 'transform 0.3s var(--ease-emphasized)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
        }}
      >
        {icon}
      </div>
    </div>
  );
}

/**
 * SpeedDial — 展开菜单容器
 * 包含所有子操作项，依次弹出动画
 */
function SpeedDial({
  actions,
  isOpen,
  expandDirection,
  onItemClick,
}: {
  actions: FabAction[];
  isOpen: boolean;
  expandDirection: 'up' | 'down' | 'left' | 'right';
  onItemClick: (action: FabAction) => void;
}) {
  /** 过滤出可见的操作项 */
  const visibleActions = actions.filter((action) => action.visible !== false);

  return (
    <div
      role="menu"
      aria-label="操作菜单"
      style={{
        position: 'absolute',
        // 根据展开方向定位：主按钮中心为原点
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        pointerEvents: isOpen ? 'auto' : 'none',
      }}
    >
      {visibleActions.map((action, index) => {
        const offset = calculateActionOffset(expandDirection, index);
        // 子项是否应显示：展开状态且在视窗内
        const shouldShow = isOpen;

        return (
          <div
            key={action.id}
            role="menuitem"
            tabIndex={shouldShow ? 0 : -1}
            aria-label={action.label}
            onClick={() => onItemClick(action)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault();
                onItemClick(action);
              }
            }}
            style={{
              position: 'absolute',
              top: '50%',
              left: '50%',
              // 展开时偏移到目标位置，收起时回到中心
              transform: shouldShow
                ? `translate(calc(-50% + ${offset.x}px), calc(-50% + ${offset.y}px))`
                : 'translate(-50%, -50%)',
              opacity: shouldShow ? 1 : 0,
              scale: shouldShow ? '1' : '0.5',
              // 依次弹出动画：每个子项延迟 ACTION_STAGGER_DELAY * index
              transition: `transform 0.3s var(--ease-emphasized) ${
                shouldShow ? index * ACTION_STAGGER_DELAY : 0
              }ms, opacity 0.2s ease ${
                shouldShow ? index * ACTION_STAGGER_DELAY : 0
              }ms, scale 0.2s ease ${
                shouldShow ? index * ACTION_STAGGER_DELAY : 0
              }ms`,
              // 收起时反向延迟（从最后一个开始收回）
              ...(shouldShow
                ? {}
                : {
                    transition: `transform 0.2s ease ${
                      (visibleActions.length - 1 - index) * ACTION_STAGGER_DELAY
                    }ms, opacity 0.15s ease ${
                      (visibleActions.length - 1 - index) * ACTION_STAGGER_DELAY
                    }ms, scale 0.15s ease ${
                      (visibleActions.length - 1 - index) * ACTION_STAGGER_DELAY
                    }ms`,
                  }),
            }}
          >
            {/* 子按钮：40px 圆形 */}
            <div
              title={action.label}
              style={{
                width: ACTION_BUTTON_SIZE,
                height: ACTION_BUTTON_SIZE,
                borderRadius: '50%',
                backgroundColor: action.color || 'var(--md-surface-container-highest)',
                color: action.color ? 'var(--md-on-primary)' : 'var(--md-on-surface)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                cursor: 'pointer',
                boxShadow: 'var(--elevation-2)',
                transition: 'background-color 0.2s, box-shadow 0.2s',
                outline: 'none',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.boxShadow = 'var(--elevation-3)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.boxShadow = 'var(--elevation-2)';
              }}
              onFocus={(e) => {
                e.currentTarget.style.boxShadow = '0 0 0 2px var(--md-primary)';
              }}
              onBlur={(e) => {
                e.currentTarget.style.boxShadow = 'var(--elevation-2)';
              }}
            >
              {action.icon}
            </div>
          </div>
        );
      })}
    </div>
  );
}

/**
 * Overlay — 半透明遮罩层
 * 点击遮罩关闭 Speed Dial
 */
function Overlay({
  isVisible,
  onClick,
}: {
  isVisible: boolean;
  onClick: () => void;
}) {
  return (
    <div
      role="presentation"
      onClick={onClick}
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.3)',
        // 使用 CSS transition 动画
        opacity: isVisible ? 1 : 0,
        pointerEvents: isVisible ? 'auto' : 'none',
        transition: 'opacity 0.3s var(--ease-default)',
        zIndex: 999,
      }}
    />
  );
}

// ==================== 主组件 ====================

/**
 * FabContainer — 浮动操作按钮容器
 *
 * @example
 * ```tsx
 * <FabContainer
 *   actions={[
 *     { id: 'add', icon: <IconPlus />, label: '新建', onClick: handleAdd },
 *     { id: 'edit', icon: <IconPencil />, label: '编辑', onClick: handleEdit },
 *     { id: 'delete', icon: <IconTrash2 />, label: '删除', onClick: handleDelete, color: 'var(--danger-500)' },
 *   ]}
 *   expandDirection="up"
 *   draggable
 * />
 * ```
 */
export function FabContainer({
  actions,
  defaultPosition = getDefaultPosition(),
  draggable = true,
  expandDirection = 'up',
  mainIcon,
}: FabContainerProps) {
  // ==================== 状态 ====================
  const [isOpen, setIsOpen] = useState(false);
  const [isMobile, setIsMobile] = useState(isMobileView());
  const [showPopover, setShowPopover] = useState(false);

  // ==================== Ref ====================
  const containerRef = useRef<HTMLDivElement>(null);

  // ==================== 拖拽集成 ====================
  const {
    position,
    isDragging,
    hasMoved,
    dragRef,
    handlers,
  } = useDraggable({
    id: 'fab-container',
    initialPosition: defaultPosition,
    elementWidth: MAIN_BUTTON_SIZE,
    elementHeight: MAIN_BUTTON_SIZE,
    onDragStart: () => {
      // 拖拽开始时关闭菜单
      setIsOpen(false);
    },
  });

  // ==================== 响应式监听 ====================
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(isMobileView());
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // ==================== 键盘事件：Esc 关闭 ====================
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && isOpen) {
        setIsOpen(false);
      }
    };

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [isOpen]);

  // ==================== 点击外部关闭 ====================
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    // 延迟绑定，避免触发当前点击
    const timerId = setTimeout(() => {
      document.addEventListener('click', handleClickOutside);
    }, 0);

    return () => {
      clearTimeout(timerId);
      document.removeEventListener('click', handleClickOutside);
    };
  }, [isOpen]);

  // ==================== 事件处理 ====================

  /** 切换菜单展开/收起，拖拽或发生移动时不触发 */
  const toggleMenu = useCallback(() => {
    if (!isDragging && !hasMoved) {
      setIsOpen((prev) => !prev);
    }
  }, [isDragging, hasMoved]);

  /** 切换预览浮窗，拖拽或发生移动时不触发 */
  const handlePreviewClick = useCallback(() => {
    if (!isDragging && !hasMoved) {
      setShowPopover((prev) => !prev);
    }
  }, [isDragging, hasMoved]);

  /** 点击子项后关闭菜单并执行回调 */
  const handleActionClick = useCallback((action: FabAction) => {
    setIsOpen(false);
    action.onClick();
  }, []);

  /** 点击遮罩关闭菜单 */
  const handleOverlayClick = useCallback(() => {
    setIsOpen(false);
  }, []);

  // ==================== 计算定位 ====================

  /** 容器定位样式 */
  const containerStyle: React.CSSProperties = isMobile
    ? {
        // 移动端：底部居中，禁止拖拽
        position: 'fixed',
        bottom: 24,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
      }
    : {
        // 桌面端：可拖拽定位
        position: 'fixed',
        left: position.x,
        top: position.y,
        zIndex: 1000,
      };

  // ==================== 渲染 ====================

  return (
    <>
      {/* 遮罩层：仅在菜单展开时显示 */}
      <Overlay isVisible={isOpen} onClick={handleOverlayClick} />

      {/* FAB 容器 */}
      <div ref={containerRef} style={containerStyle}>
        {/* Speed Dial 子菜单 */}
        <SpeedDial
          actions={actions}
          isOpen={isOpen}
          expandDirection={expandDirection}
          onItemClick={handleActionClick}
        />

        {/* 主按钮 */}
        <MainButton
          icon={mainIcon ?? <IconEye size={24} />}
          isOpen={isOpen}
          isDragging={isDragging}
          dragRef={dragRef}
          onPointerDown={draggable && !isMobile ? handlers.onPointerDown : () => {}}
          onPointerMove={draggable && !isMobile ? handlers.onPointerMove : () => {}}
          onPointerUp={draggable && !isMobile ? handlers.onPointerUp : () => {}}
          onClick={handlePreviewClick}
        />
      </div>

      {/* 预览浮窗 */}
      {showPopover && (
        <PreviewRenderer
          mode="fab-popover"
          visible={showPopover}
          onClose={() => setShowPopover(false)}
        />
      )}
    </>
}

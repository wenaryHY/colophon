/**
 * FabContainer — 全局浮动预览按钮
 *
 * 位于 AdminLayout 层，所有管理页面共享。
 * 支持拖拽、预览浮窗、无场景时关闭 FAB。
 * 配置从 FabConfigStorage 共享模块读取。
 */
import { useState, useCallback, useEffect, useRef } from 'react';
import { useDraggable } from './useDraggable';
import { calculatePopoverPositionRelativeToFabButtonRect } from './usePopoverPosition';
import { loadFabConfig } from './FabConfigStorage';
import { IconEye } from '../components/Icons';
import { PreviewRenderer } from '../preview/PreviewRenderer';
import { usePreview } from '../preview';

// ==================== 常量 ====================

/** 主按钮尺寸（px） */
const MAIN_BUTTON_SIZE = 56;

/** 移动端断点（px） */
const MOBILE_BREAKPOINT = 768;

/** 关闭菜单浮窗预估尺寸（px） */
const CLOSE_MENU_WIDTH = 180;
const CLOSE_MENU_HEIGHT = 64;

/** 惰性获取默认初始位置（右下角） */
function getDefaultPosition() {
  return { x: window.innerWidth - 80, y: window.innerHeight - 80 };
}

// ==================== 辅助函数 ====================

function isMobileView(): boolean {
  return window.innerWidth < MOBILE_BREAKPOINT;
}

// ==================== 子组件 ====================

/**
 * MainButton — 主按钮
 * 56px 圆形，眼睛图标，可拖拽，始终可交互
 */
function MainButton({
  icon,
  isDragging,
  isDraggableEnabled,
  hasScene,
  opacity,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  dragRef,
  onClick,
}: {
  icon: React.ReactNode;
  isDragging: boolean;
  isDraggableEnabled: boolean;
  hasScene: boolean;
  opacity: number;
  onPointerDown?: (e: React.PointerEvent) => void;
  onPointerMove?: (e: React.PointerEvent) => void;
  onPointerUp?: (e: React.PointerEvent) => void;
  dragRef: React.RefObject<HTMLDivElement | null>;
  onClick: () => void;
}) {
  const handleClick = useCallback(() => {
    if (!isDragging) {
      onClick();
    }
  }, [isDragging, onClick]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleClick();
      }
    },
    [handleClick],
  );

  return (
    <div
      ref={dragRef}
      role="button"
      tabIndex={0}
      aria-label="预览"
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
        cursor: isDraggableEnabled
          ? (isDragging ? 'grabbing' : 'grab')
          : (hasScene ? 'pointer' : 'default'),
        boxShadow: isDragging
          ? '0 8px 24px rgba(249, 115, 22, 0.4)'
          : 'var(--elevation-3)',
        transform: isDragging ? 'scale(1.1)' : 'scale(1)',
        transition: 'box-shadow 0.2s, transform 0.2s, opacity 0.2s',
        touchAction: 'none',
        userSelect: 'none',
        zIndex: 1001,
        opacity,
        outline: 'none',
        pointerEvents: 'auto',
      }}
    >
      {icon}
    </div>
  );
}

// ==================== 主组件 ====================

/**
 * FabContainer — 全局浮动预览按钮
 *
 * 自行从 PreviewContext 读取场景状态，从 FabConfigStorage 读取配置。
 * 无场景时可拖拽并显示关闭菜单；有场景时展开预览浮窗。
 */
export function FabContainer() {
  // ==================== 预览上下文 ====================
  const preview = usePreview();
  const hasScene = preview.sceneId !== null;

  // ==================== FAB 配置 ====================
  const [config] = useState(loadFabConfig);

  // ==================== 隐藏状态（localStorage） ====================
  const [hidden, setHidden] = useState(() => localStorage.getItem('fab-hidden') === 'true');

  useEffect(() => {
    const handler = () => setHidden(localStorage.getItem('fab-hidden') === 'true');
    window.addEventListener('fab-visibility-change', handler);
    return () => window.removeEventListener('fab-visibility-change', handler);
  }, []);

  // ==================== 状态 ====================
  const [isMobile, setIsMobile] = useState(isMobileView());
  const [showPopover, setShowPopover] = useState(false);

  // ==================== 拖拽开关（配置 & 响应式） ====================
  const isDraggableEnabled = config.draggable && !isMobile;

  // ==================== Ref ====================
  const containerRef = useRef<HTMLDivElement>(null);

  // ==================== 拖拽集成 ====================
  const {
    position,
    isDragging,
    hasMovedRef,
    dragRef,
    handlers,
  } = useDraggable({
    id: 'fab-container',
    initialPosition: getDefaultPosition(),
    elementWidth: MAIN_BUTTON_SIZE,
    elementHeight: MAIN_BUTTON_SIZE,
    snapToEdge: config.snapToEdge,
  });

  // ==================== FAB 位置追踪 ====================
  const [fabRect, setFabRect] = useState<DOMRect | null>(null);

  useEffect(() => {
    if (dragRef.current) {
      setFabRect(dragRef.current.getBoundingClientRect());
    }
  }, [position]);

  // ==================== 响应式 & FAB 位置监听（合并 window.resize） ====================
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(isMobileView());
      if (dragRef.current) {
        setFabRect(dragRef.current.getBoundingClientRect());
      }
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // ==================== 关闭菜单定位 ====================
  const closeMenuPos = hasScene
    ? null
    : calculatePopoverPositionRelativeToFabButtonRect(
        fabRect,
        CLOSE_MENU_WIDTH,
        CLOSE_MENU_HEIGHT,
      );

  // ==================== 事件处理 ====================

  const handleFabClick = useCallback(() => {
    if (!isDragging && !hasMovedRef.current) {
      setShowPopover((prev) => !prev);
    }
  }, [isDragging]);

  /** 隐藏 FAB（存入 localStorage 并触发重渲染） */
  const handleHideFab = useCallback(() => {
    localStorage.setItem('fab-hidden', 'true');
    window.dispatchEvent(new Event('fab-visibility-change'));
    setShowPopover(false);
  }, []);

  // ==================== 计算定位 ====================

  const containerStyle: React.CSSProperties = isMobile
    ? {
        position: 'fixed',
        bottom: 24,
        left: '50%',
        transform: 'translateX(-50%)',
        zIndex: 1000,
      }
    : {
        position: 'fixed',
        left: position.x,
        top: position.y,
        zIndex: 1000,
      };

  // ==================== 渲染 ====================

  // 已隐藏或配置中禁用则完全不渲染
  if (hidden || !config.enabled) return null;

  return (
    <>
      {/* FAB 容器 */}
      <div ref={containerRef} style={containerStyle}>
        <MainButton
          icon={<IconEye size={24} />}
          isDragging={isDragging}
          isDraggableEnabled={isDraggableEnabled}
          hasScene={hasScene}
          opacity={hasScene ? 1 : 0.4}
          dragRef={dragRef}
          onPointerDown={
            isDraggableEnabled ? handlers.onPointerDown : undefined
          }
          onPointerMove={
            isDraggableEnabled ? handlers.onPointerMove : undefined
          }
          onPointerUp={
            isDraggableEnabled ? handlers.onPointerUp : undefined
          }
          onClick={handleFabClick}
        />
      </div>

      {/* 预览浮窗（有场景时） */}
      {hasScene && showPopover && (
        <PreviewRenderer
          mode="fab-popover"
          visible={showPopover}
          onClose={() => setShowPopover(false)}
          fabRect={fabRect}
        />
      )}

      {/* 关闭菜单（无场景时） */}
      {!hasScene && showPopover && closeMenuPos && (
        <div
          className="fab-no-scene-menu"
          style={{
            position: 'fixed',
            left: closeMenuPos.x,
            top: closeMenuPos.y,
            background: 'var(--md-surface-container)',
            borderRadius: 'var(--radius-lg)',
            padding: '8px',
            boxShadow: 'var(--md-elevation-3)',
            minWidth: `${CLOSE_MENU_WIDTH}px`,
            zIndex: 1002,
          }}
        >
          <button
            onClick={handleHideFab}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              padding: '8px 12px',
              border: 'none',
              borderRadius: 'var(--radius-sm)',
              background: 'transparent',
              cursor: 'pointer',
              width: '100%',
              fontSize: '13px',
              color: 'var(--md-on-surface)',
            }}
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
            关闭 FAB
          </button>
        </div>
      )}
    </>
  );
}

/**
 * FabContainer — 全局浮动预览按钮
 *
 * 位于 AdminLayout 层，所有管理页面共享。
 * 当页面注册了预览场景（sceneId !== null）时按钮亮起可点击；
 * 未注册场景时按钮变灰不可点击。
 *
 * 点击展开/收起预览浮窗（fab-popover 模式）。
 */
import { useState, useCallback, useEffect, useRef } from 'react';
import { useDraggable } from './useDraggable';
import { IconEye } from '../components/Icons';
import { PreviewRenderer } from '../preview/PreviewRenderer';
import { usePreview } from '../preview';

// ==================== 常量 ====================

/** 主按钮尺寸（px） */
const MAIN_BUTTON_SIZE = 56;

/** 移动端断点（px） */
const MOBILE_BREAKPOINT = 768;

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
 * 56px 圆形，眼睛图标，可拖拽，disabled 时变灰不响应点击
 */
function MainButton({
  icon,
  isDragging,
  disabled,
  opacity,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  dragRef,
  onClick,
}: {
  icon: React.ReactNode;
  isDragging: boolean;
  disabled: boolean;
  opacity: number;
  onPointerDown: (e: React.PointerEvent) => void;
  onPointerMove: (e: React.PointerEvent) => void;
  onPointerUp: (e: React.PointerEvent) => void;
  dragRef: React.RefObject<HTMLDivElement | null>;
  onClick: () => void;
}) {
  const handleClick = useCallback(() => {
    if (!isDragging && !disabled) {
      onClick();
    }
  }, [isDragging, disabled, onClick]);

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
      tabIndex={disabled ? -1 : 0}
      aria-label="预览"
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      onPointerDown={disabled ? () => {} : onPointerDown}
      onPointerMove={disabled ? () => {} : onPointerMove}
      onPointerUp={disabled ? () => {} : onPointerUp}
      style={{
        width: MAIN_BUTTON_SIZE,
        height: MAIN_BUTTON_SIZE,
        borderRadius: '50%',
        backgroundColor: 'var(--md-primary)',
        color: 'var(--md-on-primary)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: disabled ? 'default' : isDragging ? 'grabbing' : 'pointer',
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
        pointerEvents: disabled ? 'none' : 'auto',
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
 * 不再接收 props，自行从 PreviewContext 读取场景状态。
 * 仅当页面注册了预览场景时按钮才可用。
 */
export function FabContainer() {
  // ==================== 预览上下文 ====================
  const preview = usePreview();
  const hasScene = preview.sceneId !== null;
  // ==================== 状态 ====================
  const [isMobile, setIsMobile] = useState(isMobileView());
  const [showPopover, setShowPopover] = useState(false);

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
  });

  // ==================== FAB 位置追踪 ====================
  const [fabRect, setFabRect] = useState<DOMRect | null>(null);

  useEffect(() => {
    if (dragRef.current) {
      setFabRect(dragRef.current.getBoundingClientRect());
    }
  }, [position]);

  useEffect(() => {
    const handleResize = () => {
      if (dragRef.current) {
        setFabRect(dragRef.current.getBoundingClientRect());
      }
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // ==================== 响应式监听 ====================
  useEffect(() => {
    const handleResize = () => {
      setIsMobile(isMobileView());
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // ==================== 事件处理 ====================

  const handlePreviewClick = useCallback(() => {
    if (!hasScene) return;
    if (!isDragging && !hasMovedRef.current) {
      setShowPopover((prev) => !prev);
    }
  }, [isDragging, hasScene]);

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

  return (
    <>
      {/* FAB 容器 */}
      <div ref={containerRef} style={containerStyle}>
        <MainButton
          icon={<IconEye size={24} />}
          isDragging={isDragging}
          disabled={!hasScene}
          opacity={hasScene ? 1 : 0.4}
          dragRef={dragRef}
          onPointerDown={!isMobile ? handlers.onPointerDown : () => {}}
          onPointerMove={!isMobile ? handlers.onPointerMove : () => {}}
          onPointerUp={!isMobile ? handlers.onPointerUp : () => {}}
          onClick={handlePreviewClick}
        />
      </div>

      {/* 预览浮窗 */}
      {showPopover && (
        <PreviewRenderer
          mode="fab-popover"
          visible={showPopover}
          onClose={() => setShowPopover(false)}
          fabRect={fabRect}
        />
      )}
    </>
  );
}

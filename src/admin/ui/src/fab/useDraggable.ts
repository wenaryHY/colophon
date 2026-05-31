import { useState, useCallback, useRef, useEffect } from 'react';

// ==================== 类型定义 ====================

/** 可拖拽 Hook 的配置选项 */
export interface UseDraggableOptions {
  /** 唯一标识，用于持久化存储位置 */
  id: string;
  /** 初始位置 */
  initialPosition: { x: number; y: number };
  /** 元素宽度（像素），默认 56px */
  elementWidth?: number;
  /** 元素高度（像素），默认 56px */
  elementHeight?: number;
  /** 是否吸附到视窗边缘，默认 true */
  snapToEdge?: boolean;
  /** 吸附触发阈值（像素），默认 24px */
  snapThreshold?: number;
  /** 拖拽开始时的回调 */
  onDragStart?: () => void;
  /** 拖拽结束时的回调，返回最终位置 */
  onDragEnd?: (position: { x: number; y: number }) => void;
}

/** 可拖拽 Hook 的返回值 */
export interface UseDraggableReturn {
  /** 当前位置 */
  position: { x: number; y: number };
  /** 是否正在拖拽 */
  isDragging: boolean;
  /** 绑定到拖拽元素的 ref */
  dragRef: React.RefObject<HTMLDivElement>;
  /** 需要绑定到元素的事件处理器 */
  handlers: {
    onPointerDown: (e: React.PointerEvent) => void;
    onPointerMove: (e: React.PointerEvent) => void;
    onPointerUp: (e: React.PointerEvent) => void;
  };
}

// ==================== 工具函数 ====================

/**
 * 将坐标限制在视窗范围内，防止元素被拖出屏幕
 * @param x - 目标 X 坐标
 * @param y - 目标 Y 坐标
 * @param elementWidth - 元素宽度
 * @param elementHeight - 元素高度
 * @returns 限制后的坐标
 */
function clampToViewport(
  x: number,
  y: number,
  elementWidth: number,
  elementHeight: number
): { x: number; y: number } {
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  return {
    x: Math.max(0, Math.min(x, vw - elementWidth)),
    y: Math.max(0, Math.min(y, vh - elementHeight)),
  };
}

/**
 * 磁性吸附算法：当元素靠近视窗边缘时自动吸附
 * @param x - 当前 X 坐标
 * @param y - 当前 Y 坐标
 * @param elementWidth - 元素宽度
 * @param elementHeight - 元素高度
 * @param threshold - 吸附触发阈值
 * @returns 吸附后的坐标
 */
function snapToEdge(
  x: number,
  y: number,
  elementWidth: number,
  elementHeight: number,
  threshold: number
): { x: number; y: number } {
  const vw = window.innerWidth;
  const vh = window.innerHeight;

  let newX = x;
  let newY = y;

  // 左边缘吸附：距离左边缘小于阈值时，吸附到距离边缘 16px 的位置
  if (x < threshold) newX = 16;
  // 右边缘吸附：距离右边缘小于阈值时
  if (x > vw - elementWidth - threshold) newX = vw - elementWidth - 16;
  // 上边缘吸附：距离上边缘小于阈值时
  if (y < threshold) newY = 16;
  // 下边缘吸附：距离下边缘小于阈值时
  if (y > vh - elementHeight - threshold) newY = vh - elementHeight - 16;

  return { x: newX, y: newY };
}

/**
 * 从 localStorage 恢复持久化的位置
 * @param id - 元素唯一标识
 * @param initialPosition - 初始位置（作为后备）
 * @param elementWidth - 元素宽度
 * @param elementHeight - 元素高度
 * @returns 恢复后的位置
 */
function restorePosition(
  id: string,
  initialPosition: { x: number; y: number },
  elementWidth: number,
  elementHeight: number
): { x: number; y: number } {
  try {
    const raw = localStorage.getItem(`fab-position-${id}`);
    if (raw) {
      const saved = JSON.parse(raw) as { x: number; y: number };
      // 校验恢复的位置是否在当前视窗内，防止显示器分辨率变化后元素丢失
      const clamped = clampToViewport(saved.x, saved.y, elementWidth, elementHeight);
      // 如果位置被 clamp 修正了，说明原位置已超出视窗，更新存储
      if (clamped.x !== saved.x || clamped.y !== saved.y) {
        localStorage.setItem(`fab-position-${id}`, JSON.stringify(clamped));
      }
      return clamped;
    }
  } catch {
    // localStorage 不可用或数据损坏时忽略，使用初始位置
  }
  return initialPosition;
}

/**
 * 将位置持久化到 localStorage
 * @param id - 元素唯一标识
 * @param position - 要保存的位置
 */
function savePosition(id: string, position: { x: number; y: number }): void {
  try {
    localStorage.setItem(`fab-position-${id}`, JSON.stringify(position));
  } catch {
    // 存储满或隐私模式下忽略
  }
}

// ==================== Hook 实现 ====================

/**
 * 可拖拽元素 Hook
 *
 * 支持：
 * - 鼠标和触摸事件统一处理（Pointer Events）
 * - 边界检测，防止元素被拖出视窗
 * - 磁性吸附到视窗边缘
 * - 位置持久化到 localStorage
 * - 使用 requestAnimationFrame 优化渲染性能
 *
 * @example
 * ```tsx
 * const { position, isDragging, dragRef, handlers } = useDraggable({
 *   id: 'fab-main',
 *   initialPosition: { x: window.innerWidth - 80, y: window.innerHeight - 80 },
 * });
 *
 * return (
 *   <div
 *     ref={dragRef}
 *     style={{ position: 'fixed', left: position.x, top: position.y, touchAction: 'none' }}
 *     {...handlers}
 *   >
 *     拖拽我
 *   </div>
 * );
 * ```
 */
export function useDraggable(options: UseDraggableOptions): UseDraggableReturn {
  const {
    id,
    initialPosition,
    elementWidth = 56,
    elementHeight = 56,
    snapToEdge: enableSnap = true,
    snapThreshold = 24,
    onDragStart,
    onDragEnd,
  } = options;

  // ---- 状态 ----
  const [position, setPosition] = useState<{ x: number; y: number }>(() =>
    restorePosition(id, initialPosition, elementWidth, elementHeight)
  );
  const [isDragging, setIsDragging] = useState(false);

  // ---- Ref ----
  const dragRef = useRef<HTMLDivElement>(null);

  // 使用 ref 存储拖拽过程中的中间状态，避免频繁 setState
  const dragStateRef = useRef({
    /** 拖拽开始时指针在元素内的偏移量 */
    startX: 0,
    startY: 0,
    /** 拖拽开始时元素的左上角位置 */
    initialX: 0,
    initialY: 0,
    /** 当前 rAF 是否已调度 */
    rafId: 0,
    /** 最新计算出的位置（供 rAF 回调使用） */
    latestX: 0,
    latestY: 0,
  });

  // ---- 事件处理 ----

  /**
   * 指针按下：开始拖拽
   * - 记录指针在元素内的偏移量
   * - 使用 setPointerCapture 确保后续 move/up 事件始终发送到该元素
   */
  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      // 只处理主按钮（左键/第一触控点）
      if (e.button !== 0) return;

      const rect = (e.target as HTMLElement).getBoundingClientRect();
      const state = dragStateRef.current;

      state.startX = e.clientX - rect.left;
      state.startY = e.clientY - rect.top;
      state.initialX = position.x;
      state.initialY = position.y;

      setIsDragging(true);
      onDragStart?.();

      // 捕获指针，后续 move/up 事件即使离开元素也会触发
      (e.target as HTMLElement).setPointerCapture(e.pointerId);

      // 阻止浏览器默认的拖拽行为（如文本选择、链接拖拽）
      e.preventDefault();
    },
    [position.x, position.y, onDragStart]
  );

  /**
   * 指针移动：更新位置
   * - 计算新位置并应用边界检测
   * - 使用 requestAnimationFrame 合并高频事件，避免重复渲染
   */
  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!isDragging) return;

      const state = dragStateRef.current;

      // 计算新位置 = 指针当前位置 - 指针在元素内的偏移量
      let newX = e.clientX - state.startX;
      let newY = e.clientY - state.startY;

      // 先做边界限制，确保不会拖出视窗
      const clamped = clampToViewport(newX, newY, elementWidth, elementHeight);
      newX = clamped.x;
      newY = clamped.y;

      // 存储到 ref，供 rAF 回调使用
      state.latestX = newX;
      state.latestY = newY;

      // 如果还没有调度 rAF，则调度一帧
      if (!state.rafId) {
        state.rafId = requestAnimationFrame(() => {
          state.rafId = 0;
          setPosition({ x: state.latestX, y: state.latestY });
        });
      }

      e.preventDefault();
    },
    [isDragging, elementWidth, elementHeight]
  );

  /**
   * 指针抬起：结束拖拽
   * - 执行磁性吸附（如果启用）
   * - 持久化最终位置
   * - 清理状态
   */
  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      if (!isDragging) return;

      const state = dragStateRef.current;

      // 取消可能还在排队的 rAF
      if (state.rafId) {
        cancelAnimationFrame(state.rafId);
        state.rafId = 0;
      }

      // 最终位置
      let finalX = state.latestX;
      let finalY = state.latestY;

      // 磁性吸附：靠近边缘时自动吸附
      if (enableSnap) {
        const snapped = snapToEdge(finalX, finalY, elementWidth, elementHeight, snapThreshold);
        finalX = snapped.x;
        finalY = snapped.y;
      }

      // 确保最终位置仍在视窗内
      const finalClamped = clampToViewport(finalX, finalY, elementWidth, elementHeight);
      finalX = finalClamped.x;
      finalY = finalClamped.y;

      // 更新状态并持久化
      setPosition({ x: finalX, y: finalY });
      savePosition(id, { x: finalX, y: finalY });
      setIsDragging(false);

      onDragEnd?.({ x: finalX, y: finalY });

      e.preventDefault();
    },
    [isDragging, id, elementWidth, elementHeight, enableSnap, snapThreshold, onDragEnd]
  );

  // ---- 清理 rAF ----
  useEffect(() => {
    return () => {
      const state = dragStateRef.current;
      if (state.rafId) {
        cancelAnimationFrame(state.rafId);
      }
    };
  }, []);

  return {
    position,
    isDragging,
    dragRef,
    handlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
    },
  };
}

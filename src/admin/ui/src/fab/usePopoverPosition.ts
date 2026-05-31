/**
 * 根据 FAB 位置计算浮窗坐标
 * 优先级：下方 → 上方 → 右侧 → 左侧 → 粘底兜底
 *
 * 阶段3将完善完整的定位算法，当前返回默认位置
 */

interface PopoverPosition {
  x: number;
  y: number;
  anchor: 'below' | 'above' | 'right' | 'left' | 'bottom-sticky';
}

/** 浮窗默认尺寸常量 */
const POPOVER_DEFAULT_WIDTH = 420;
const POPOVER_DEFAULT_HEIGHT = 560;

/** 屏幕边距 */
const SCREEN_MARGIN = 16;

/**
 * 计算浮窗位置
 * @param fabRect - FAB 按钮的 DOMRect，为 null 时使用默认位置
 * @param popoverWidth - 浮窗宽度
 * @param popoverHeight - 浮窗高度
 */
export function usePopoverPosition(
  fabRect: DOMRect | null,
  popoverWidth: number = POPOVER_DEFAULT_WIDTH,
  popoverHeight: number = POPOVER_DEFAULT_HEIGHT,
): PopoverPosition {
  // 阶段3完善：根据 fabRect 和可视区域计算最佳方位
  // 当前返回默认位置（右下角）
  const viewportWidth = typeof window !== 'undefined' ? window.innerWidth : 1024;
  const viewportHeight = typeof window !== 'undefined' ? window.innerHeight : 768;

  if (fabRect) {
    // 下方是否够放
    const spaceBelow = viewportHeight - fabRect.bottom - SCREEN_MARGIN;
    if (spaceBelow >= popoverHeight) {
      return {
        x: Math.max(SCREEN_MARGIN, fabRect.right - popoverWidth),
        y: fabRect.bottom + SCREEN_MARGIN,
        anchor: 'below',
      };
    }

    // 上方是否够放
    const spaceAbove = fabRect.top - SCREEN_MARGIN;
    if (spaceAbove >= popoverHeight) {
      return {
        x: Math.max(SCREEN_MARGIN, fabRect.right - popoverWidth),
        y: fabRect.top - popoverHeight - SCREEN_MARGIN,
        anchor: 'above',
      };
    }

    // 右侧是否够放
    const spaceRight = viewportWidth - fabRect.right - SCREEN_MARGIN;
    if (spaceRight >= popoverWidth) {
      return {
        x: fabRect.right + SCREEN_MARGIN,
        y: Math.max(SCREEN_MARGIN, fabRect.top - popoverHeight / 2),
        anchor: 'right',
      };
    }

    // 左侧是否够放
    const spaceLeft = fabRect.left - SCREEN_MARGIN;
    if (spaceLeft >= popoverWidth) {
      return {
        x: fabRect.left - popoverWidth - SCREEN_MARGIN,
        y: Math.max(SCREEN_MARGIN, fabRect.top - popoverHeight / 2),
        anchor: 'left',
      };
    }
  }

  // 兜底：粘底显示
  return {
    x: viewportWidth - popoverWidth - SCREEN_MARGIN,
    y: viewportHeight - popoverHeight - SCREEN_MARGIN,
    anchor: 'bottom-sticky',
  };
}

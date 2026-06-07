import {
  useState,
  useRef,
  useEffect,
  useMemo,
  useCallback,
  forwardRef,
  useImperativeHandle,
  type ReactNode,
  type CSSProperties,
  type Ref,
  type KeyboardEvent as ReactKeyboardEvent,
} from 'react';
import { createPortal } from 'react-dom';

// ── 类型定义 ──

interface ParsedOptionItem {
  value: string;
  labelText: string;
  originalChildren: ReactNode;
}

interface SelectProps {
  label?: string;
  error?: string;
  value?: string | number | readonly string[];
  onChange?: (e: { target: { value: string } }) => void;
  disabled?: boolean;
  children?: ReactNode;
  className?: string;
  style?: CSSProperties;
  id?: string;
}

// ── 从 children 中递归提取 <option> 元素 ──

function extractTextFromReactNode(node: ReactNode): string {
  if (node == null || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(extractTextFromReactNode).join('');
  if (typeof node === 'object' && 'props' in node) {
    return extractTextFromReactNode((node as { props: { children?: ReactNode } }).props.children);
  }
  return '';
}

function parseOptionsFromChildren(children: ReactNode): ParsedOptionItem[] {
  const result: ParsedOptionItem[] = [];
  const stack: ReactNode[] = Array.isArray(children) ? [...children] : [children];

  while (stack.length > 0) {
    const item = stack.pop();
    if (item == null || typeof item === 'boolean') continue;
    if (typeof item === 'string' || typeof item === 'number') continue;
    if (Array.isArray(item)) {
      for (let i = item.length - 1; i >= 0; i--) stack.push(item[i]);
      continue;
    }
    if (typeof item === 'object' && 'type' in item && 'props' in item) {
      const el = item as { type: unknown; props: { value?: unknown; children?: ReactNode } };
      if (el.type === 'option') {
        result.push({
          value: String(el.props.value ?? ''),
          labelText: extractTextFromReactNode(el.props.children),
          originalChildren: el.props.children,
        });
      } else if (el.props.children) {
        stack.push(el.props.children);
      }
    }
  }
  return result;
}

// ── MD3 自定义下拉 Select ──

const SELECT_TRIGGER_STYLE: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  width: '100%',
  height: '40px',
  padding: '0 14px',
  border: '1px solid var(--md-outline-variant)',
  borderRadius: '10px',
  fontSize: '14px',
  color: 'var(--md-on-surface)',
  background: 'var(--md-surface-container-highest)',
  cursor: 'pointer',
  outline: 'none',
  transition: 'border-color 0.2s ease, box-shadow 0.2s ease',
  boxSizing: 'border-box',
  userSelect: 'none',
  fontFamily: 'inherit',
  gap: '8px',
};

const SELECT_TRIGGER_DISABLED_STYLE: CSSProperties = {
  opacity: 0.45,
  cursor: 'not-allowed',
};

const SELECT_TRIGGER_FOCUS_STYLE: CSSProperties = {
  borderColor: 'var(--md-primary)',
  boxShadow: '0 0 0 2px rgba(249,115,22,0.18)',
};

const SELECT_TRIGGER_PLACEHOLDER_STYLE: CSSProperties = {
  color: 'var(--md-outline)',
  opacity: 0.7,
};

const DROPDOWN_CONTAINER_STYLE: CSSProperties = {
  position: 'fixed',
  zIndex: 9999,
  minWidth: '120px',
  maxHeight: '280px',
  overflowY: 'auto',
  background: 'var(--md-surface-container-highest)',
  border: '1px solid var(--md-outline-variant)',
  borderRadius: '10px',
  boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
  padding: '4px 0',
  animation: 'scaleIn 0.12s ease-out both',
};

const DROPDOWN_OPTION_STYLE: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  padding: '10px 14px',
  fontSize: '14px',
  color: 'var(--md-on-surface)',
  cursor: 'pointer',
  transition: 'background 0.1s ease',
  userSelect: 'none',
};

const DROPDOWN_OPTION_SELECTED_STYLE: CSSProperties = {
  background: 'var(--md-surface-container-low)',
  fontWeight: 600,
};

const DROPDOWN_OPTION_HIGHLIGHTED_STYLE: CSSProperties = {
  background: 'var(--md-surface-container-high)',
};

const LABEL_STYLE: CSSProperties = {
  fontSize: '13px',
  fontWeight: 600,
  color: 'var(--md-on-surface-variant)',
  letterSpacing: '0.01em',
};

const WRAPPER_STYLE: CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '6px',
  position: 'relative',
};

const CHEVRON_SVG = (
  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--md-on-surface-variant)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <polyline points="6 9 12 15 18 9" />
  </svg>
);

export const Select = forwardRef<HTMLDivElement, SelectProps>(
  ({ label, error, value, onChange, disabled, children, className, style, id }, ref) => {
    const [isOpen, setIsOpen] = useState(false);
    const [highlightedIndex, setHighlightedIndex] = useState(-1);
    const triggerRef = useRef<HTMLDivElement>(null);
    const dropdownRef = useRef<HTMLDivElement>(null);

    useImperativeHandle(ref, () => triggerRef.current as HTMLDivElement);

    const options = useMemo(() => parseOptionsFromChildren(children), [children]);

    const selectedOption = options.find((o) => o.value === String(value));

    const stringValue = String(value ?? '');

    // ── 关闭下拉 ──
    const closeDropdown = useCallback(() => {
      setIsOpen(false);
      setHighlightedIndex(-1);
    }, []);

    // ── 选择选项 ──
    const selectOption = useCallback(
      (optionValue: string) => {
        onChange?.({ target: { value: optionValue } });
        closeDropdown();
        triggerRef.current?.focus();
      },
      [onChange, closeDropdown],
    );

    // ── 打开下拉时锁住 body 滚动，避免页面跟随下拉框滚动 ──
    useEffect(() => {
      if (!isOpen) return;
      const previousOverflow = document.body.style.overflow;
      document.body.style.overflow = 'hidden';
      return () => {
        document.body.style.overflow = previousOverflow;
      };
    }, [isOpen]);

    // ── 点击外部关闭 ──
    useEffect(() => {
      if (!isOpen) return;

      const handleMouseDown = (e: globalThis.MouseEvent) => {
        const target = e.target as HTMLElement;
        if (triggerRef.current?.contains(target)) return;
        if (dropdownRef.current?.contains(target)) return;
        closeDropdown();
      };

      document.addEventListener('mousedown', handleMouseDown);
      return () => document.removeEventListener('mousedown', handleMouseDown);
    }, [isOpen, closeDropdown]);

    // ── 滚动、窗口尺寸变化关闭 ──
    useEffect(() => {
      if (!isOpen) return;

      const handleScroll = () => closeDropdown();
      const handleResize = () => closeDropdown();
      window.addEventListener('scroll', handleScroll, true);
      window.addEventListener('resize', handleResize);
      return () => {
        window.removeEventListener('scroll', handleScroll, true);
        window.removeEventListener('resize', handleResize);
      };
    }, [isOpen, closeDropdown]);

    // ── 键盘导航 ──
    const handleKeyDown = useCallback(
      (e: ReactKeyboardEvent<HTMLDivElement>) => {
        if (disabled || options.length === 0) return;

        if (!isOpen) {
          if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            setIsOpen(true);
            const idx = options.findIndex((o) => o.value === stringValue);
            setHighlightedIndex(idx >= 0 ? idx : 0);
          }
          return;
        }

        switch (e.key) {
          case 'ArrowDown':
            e.preventDefault();
            setHighlightedIndex((prev) => (prev + 1) % options.length);
            break;
          case 'ArrowUp':
            e.preventDefault();
            setHighlightedIndex((prev) => (prev - 1 + options.length) % options.length);
            break;
          case 'Enter':
          case ' ':
            e.preventDefault();
            if (highlightedIndex >= 0 && highlightedIndex < options.length) {
              selectOption(options[highlightedIndex].value);
            }
            break;
          case 'Escape':
            e.preventDefault();
            closeDropdown();
            triggerRef.current?.focus();
            break;
          case 'Tab':
            closeDropdown();
            break;
        }
      },
      [disabled, isOpen, options, highlightedIndex, selectOption, closeDropdown],
    );

    // ── 焦点样式 ──
    const [isFocused, setIsFocused] = useState(false);

    // ── 计算下拉位置 ──
    const [dropdownStyle, setDropdownStyle] = useState<CSSProperties>({});

    useEffect(() => {
      if (!isOpen || !triggerRef.current) return;

      const triggerRect = triggerRef.current.getBoundingClientRect();
      const dropdownHeight = 280; // max height
      const dropdownMinWidth = triggerRect.width;
      const viewportHeight = window.innerHeight;
      const viewportWidth = window.innerWidth;

      const spaceBelow = viewportHeight - triggerRect.bottom;
      const spaceAbove = triggerRect.top;

      let top: number;
      if (spaceBelow >= dropdownHeight || spaceBelow >= spaceAbove) {
        // 下方空间足够 → 展开在下方
        top = triggerRect.bottom + 4;
      } else {
        // 上方空间更大 → 展开在上方
        top = triggerRect.top - dropdownHeight - 4;
      }

      // 不超出左侧
      let left = triggerRect.left;
      if (left + dropdownMinWidth > viewportWidth) {
        left = viewportWidth - dropdownMinWidth - 8;
      }
      if (left < 8) left = 8;

      setDropdownStyle({
        top,
        left,
        minWidth: dropdownMinWidth,
      });
    }, [isOpen, stringValue]);

    return (
      <div style={{ ...WRAPPER_STYLE, ...style }} className={className}>
        {label && (
          <label htmlFor={id} style={LABEL_STYLE}>
            {label}
          </label>
        )}

        {/* ── 触发器 ── */}
        <div
          ref={triggerRef}
          id={id}
          role="combobox"
          aria-expanded={isOpen}
          aria-haspopup="listbox"
          tabIndex={disabled ? -1 : 0}
          onClick={() => {
            if (disabled || options.length === 0) return;
            setIsOpen((prev) => !prev);
            if (!isOpen) {
              const idx = options.findIndex((o) => o.value === stringValue);
              setHighlightedIndex(idx >= 0 ? idx : 0);
            }
          }}
          onKeyDown={handleKeyDown}
          onFocus={() => setIsFocused(true)}
          onBlur={(e) => {
            // 仅在焦点离开组件时重置
            if (!triggerRef.current?.contains(e.relatedTarget as Node)) {
              setIsFocused(false);
            }
          }}
          style={{
            ...SELECT_TRIGGER_STYLE,
            ...(disabled ? SELECT_TRIGGER_DISABLED_STYLE : {}),
            ...(isFocused ? SELECT_TRIGGER_FOCUS_STYLE : {}),
            ...(error ? { borderColor: 'var(--md-error)' } : {}),
          }}
        >
          <span
            style={{
              flex: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
              ...(selectedOption ? {} : SELECT_TRIGGER_PLACEHOLDER_STYLE),
            }}
          >
            {selectedOption ? selectedOption.labelText : '\u00A0'}
          </span>
          <span
            style={{
              display: 'flex',
              alignItems: 'center',
              transition: 'transform 0.2s ease',
              transform: isOpen ? 'rotate(180deg)' : 'rotate(0deg)',
              flexShrink: 0,
            }}
          >
            {CHEVRON_SVG}
          </span>
        </div>

        {/* ── 下拉弹出层（Portal 到 body） ── */}
        {isOpen && options.length > 0 && (
          <DropdownPortalInstance
            dropdownRef={dropdownRef}
            dropdownStyle={dropdownStyle}
          >
            {options.map((opt, idx) => {
              const isSelected = opt.value === stringValue;
              const isHighlighted = idx === highlightedIndex;
              return (
                <div
                  key={opt.value}
                  role="option"
                  aria-selected={isSelected}
                  style={{
                    ...DROPDOWN_OPTION_STYLE,
                    ...(isSelected ? DROPDOWN_OPTION_SELECTED_STYLE : {}),
                    ...(isHighlighted ? DROPDOWN_OPTION_HIGHLIGHTED_STYLE : {}),
                  }}
                  onMouseEnter={() => setHighlightedIndex(idx)}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                  }}
                  onClick={(e) => {
                    e.stopPropagation();
                    selectOption(opt.value);
                  }}
                >
                  {opt.originalChildren ?? opt.labelText}
                </div>
              );
            })}
          </DropdownPortalInstance>
        )}
      </div>
    );
  },
);

Select.displayName = 'Select';

// ── Portal 辅助组件 ──
// 使用 createPortal 将下拉渲染到 document.body，避免被父级 overflow/clip 裁剪

interface DropdownPortalInstanceProps {
  dropdownRef: Ref<HTMLDivElement>;
  dropdownStyle: CSSProperties;
  children: ReactNode;
}

function DropdownPortalInstance({
  dropdownRef,
  dropdownStyle,
  children,
}: DropdownPortalInstanceProps) {
  const containerRef = useRef<HTMLDivElement | null>(null);

  // 惰性创建容器 div，仅一次
  if (!containerRef.current && typeof document !== 'undefined') {
    containerRef.current = document.createElement('div');
  }

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    document.body.appendChild(el);
    return () => {
      if (el.parentNode) {
        el.parentNode.removeChild(el);
      }
    };
  }, []);

  if (!containerRef.current) return null;

  return createPortal(
    <div
      ref={dropdownRef}
      role="listbox"
      style={{ ...DROPDOWN_CONTAINER_STYLE, ...dropdownStyle }}
      onWheel={(e) => e.stopPropagation()}
    >
      {children}
    </div>,
    containerRef.current,
  );
}

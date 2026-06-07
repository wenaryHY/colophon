import { useState, useRef, useEffect, useCallback, type CSSProperties } from 'react';
import { createPortal } from 'react-dom';
import { useI18n } from '../i18n';

// ── 零状态时间选择器 —— 完全由 props 驱动。 ──
// ── 以 Rust trait bound 思维设计：改变行为 = 改变调用参数，不改组件代码。 ──

interface TimePickerProps {
  hour: number;
  minute: number;
  onChange: (hour: number, minute: number) => void;
  minHour?: number;
  maxHour?: number;
  minuteStep?: number;
  disabled?: boolean;
}

// ── 工具：生成 0-23 的小时列表 ──
const TIME_PICKER_HOURS_FROM_MIDNIGHT_TO_END_OF_DAY = Array.from({ length: 24 }, (_, i) => i);

// ── 自定义下拉常量样式 ──

const DROPDOWN_TRIGGER_STYLE: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: '56px',
  height: '36px',
  padding: '0 6px',
  border: '1px solid var(--md-outline-variant)',
  borderRadius: '10px',
  fontSize: '14px',
  fontFamily: 'inherit',
  color: 'var(--md-on-surface)',
  background: 'var(--md-surface-container-highest)',
  cursor: 'pointer',
  outline: 'none',
  transition: 'border-color 0.2s ease, box-shadow 0.2s ease',
  userSelect: 'none',
  boxSizing: 'border-box',
  textAlign: 'center',
};

const DROPDOWN_TRIGGER_DISABLED: CSSProperties = {
  opacity: 0.4,
  cursor: 'not-allowed',
};

const DROPDOWN_TRIGGER_FOCUSED: CSSProperties = {
  borderColor: 'var(--md-primary)',
  boxShadow: '0 0 0 2px rgba(249,115,22,0.18)',
};

const DROPDOWN_LIST_STYLE: CSSProperties = {
  position: 'fixed',
  zIndex: 9999,
  width: '64px',
  maxHeight: '240px',
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
  justifyContent: 'center',
  padding: '6px 8px',
  fontSize: '13px',
  color: 'var(--md-on-surface)',
  cursor: 'pointer',
  transition: 'background 0.1s ease',
  userSelect: 'none',
};

const DROPDOWN_OPTION_SELECTED: CSSProperties = {
  background: 'var(--md-surface-container-low)',
  fontWeight: 700,
  color: 'var(--md-primary)',
};

const DROPDOWN_OPTION_HIGHLIGHTED: CSSProperties = {
  background: 'var(--md-surface-container-high)',
};

// ── 简易自定义下拉（专为时间选择设计） ──

interface TimeUnitDropdownProps {
  value: number;
  options: number[];
  formatValue: (v: number) => string;
  onChange: (v: number) => void;
  disabled: boolean;
}

function TimeUnitDropdown({ value, options, formatValue, onChange, disabled }: TimeUnitDropdownProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [highlightedIndex, setHighlightedIndex] = useState(-1);
  const [isFocused, setIsFocused] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const portalContainerRef = useRef<HTMLDivElement | null>(null);
  const isMouseInsideRef = useRef(false);
  const selectedIndex = options.indexOf(value);

  // 惰性 portal 容器
  if (!portalContainerRef.current && typeof document !== 'undefined') {
    portalContainerRef.current = document.createElement('div');
  }

  useEffect(() => {
    const el = portalContainerRef.current;
    if (!el) return;
    document.body.appendChild(el);
    return () => {
      if (el.parentNode) el.parentNode.removeChild(el);
    };
  }, []);

  const close = useCallback(() => {
    setIsOpen(false);
    setHighlightedIndex(-1);
  }, []);

  const select = useCallback(
    (v: number) => {
      onChange(v);
      close();
      triggerRef.current?.focus();
    },
    [onChange, close],
  );

  // 点击外部关闭
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      if (isMouseInsideRef.current) return;
      if (triggerRef.current?.contains(e.target as Node)) return;
      if (listRef.current?.contains(e.target as Node)) return;
      close();
    };
    document.addEventListener('mousedown', handler, true);
    return () => document.removeEventListener('mousedown', handler, true);
  }, [isOpen, close]);

  // 滚动关闭
  useEffect(() => {
    if (!isOpen) return;
    const handler = () => close();
    window.addEventListener('scroll', handler, true);
    return () => window.removeEventListener('scroll', handler, true);
  }, [isOpen, close]);

  // 键盘
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (disabled) return;
      if (!isOpen) {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
          e.preventDefault();
          setIsOpen(true);
          setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
        }
        return;
      }
      switch (e.key) {
        case 'ArrowDown':
          e.preventDefault();
          setHighlightedIndex((p) => (p + 1) % options.length);
          break;
        case 'ArrowUp':
          e.preventDefault();
          setHighlightedIndex((p) => (p - 1 + options.length) % options.length);
          break;
        case 'Enter':
        case ' ':
          e.preventDefault();
          if (highlightedIndex >= 0) select(options[highlightedIndex]);
          break;
        case 'Escape':
          e.preventDefault();
          close();
          triggerRef.current?.focus();
          break;
        case 'Tab':
          close();
          break;
      }
    },
    [disabled, isOpen, options, highlightedIndex, selectedIndex, select, close],
  );

  // 下拉位置计算
  const [posStyle, setPosStyle] = useState<CSSProperties>({});

  useEffect(() => {
    if (!isOpen || !triggerRef.current) return;
    const rect = triggerRef.current.getBoundingClientRect();
    const listHeight = 240;
    const spaceBelow = window.innerHeight - rect.bottom;
    const top = spaceBelow >= listHeight || spaceBelow >= rect.top
      ? rect.bottom + 4
      : rect.top - listHeight - 4;
    let left = rect.left;
    if (left + 64 > window.innerWidth) left = window.innerWidth - 64 - 8;
    if (left < 8) left = 8;
    setPosStyle({ top, left });
  }, [isOpen, value]);

  // 自动滚到选中项
  useEffect(() => {
    if (!isOpen || !listRef.current) return;
    const idx = highlightedIndex >= 0 ? highlightedIndex : selectedIndex;
    const el = listRef.current.children[idx] as HTMLElement | undefined;
    el?.scrollIntoView({ block: 'nearest' });
  }, [isOpen, highlightedIndex, selectedIndex]);

  return (
    <>
      <button
        ref={triggerRef}
        type="button"
        disabled={disabled}
        style={{
          ...DROPDOWN_TRIGGER_STYLE,
          ...(disabled ? DROPDOWN_TRIGGER_DISABLED : {}),
          ...(isFocused ? DROPDOWN_TRIGGER_FOCUSED : {}),
        }}
        onClick={() => {
          if (disabled) return;
          setIsOpen((p) => !p);
          if (!isOpen) setHighlightedIndex(selectedIndex >= 0 ? selectedIndex : 0);
        }}
        onKeyDown={handleKeyDown}
        onFocus={() => setIsFocused(true)}
        onBlur={() => setIsFocused(false)}
        aria-haspopup="listbox"
        aria-expanded={isOpen}
      >
        {formatValue(value)}
      </button>

      {isOpen &&
        portalContainerRef.current &&
        createPortal(
          <div
            ref={listRef}
            role="listbox"
            style={{ ...DROPDOWN_LIST_STYLE, ...posStyle }}
            onMouseEnter={() => { isMouseInsideRef.current = true; }}
            onMouseLeave={() => { isMouseInsideRef.current = false; }}
          >
            {options.map((opt, idx) => {
              const isSel = opt === value;
              const isHi = idx === highlightedIndex;
              return (
                <div
                  key={opt}
                  role="option"
                  aria-selected={isSel}
                  style={{
                    ...DROPDOWN_OPTION_STYLE,
                    ...(isSel ? DROPDOWN_OPTION_SELECTED : {}),
                    ...(isHi ? DROPDOWN_OPTION_HIGHLIGHTED : {}),
                  }}
                  onMouseEnter={() => setHighlightedIndex(idx)}
                  onMouseDown={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                  }}
                  onClick={(e) => {
                    e.stopPropagation();
                    select(opt);
                  }}
                >
                  {formatValue(opt)}
                </div>
              );
            })}
          </div>,
          portalContainerRef.current,
        )}
    </>
  );
}

// ── TimePicker 主组件 ──

export function TimePicker({
  hour,
  minute,
  onChange,
  minHour = 0,
  maxHour = 23,
  minuteStep = 1,
  disabled = false,
}: TimePickerProps) {
  const { t } = useI18n();

  const availableMinutes = Array.from(
    { length: Math.ceil(60 / minuteStep) },
    (_, i) => i * minuteStep,
  );

  const formatTwoDigit = (v: number) => String(v).padStart(2, '0');

  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
      <TimeUnitDropdown
        value={hour}
        options={TIME_PICKER_HOURS_FROM_MIDNIGHT_TO_END_OF_DAY.filter((h) => h >= minHour && h <= maxHour)}
        formatValue={formatTwoDigit}
        onChange={(h) => onChange(h, minute)}
        disabled={disabled}
      />
      <span style={{ color: 'var(--md-outline)', fontSize: 14, fontWeight: 500 }}>:</span>
      <TimeUnitDropdown
        value={minute}
        options={availableMinutes}
        formatValue={formatTwoDigit}
        onChange={(m) => onChange(hour, m)}
        disabled={disabled}
      />
      <span style={{ fontSize: 12, color: 'var(--md-outline)' }}>{t('timeHourAbbreviation')}</span>
    </div>
  );
}

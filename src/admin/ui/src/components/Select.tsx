import { type ReactNode, type CSSProperties, useId } from 'react';
import * as RadixSelect from '@radix-ui/react-select';

// ── 类型定义 ──

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

// ── MD3 样式常量 ──

const TRIGGER_STYLE: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  width: '100%',
  padding: '10px 14px',
  border: '1px solid var(--md-outline-variant)',
  borderRadius: 10,
  background: 'var(--md-surface-container-highest)',
  color: 'var(--md-on-surface)',
  fontSize: 14,
  fontFamily: 'inherit',
  cursor: 'pointer',
  outline: 'none',
  gap: 8,
  boxSizing: 'border-box',
};

const TRIGGER_DISABLED_STYLE: CSSProperties = {
  opacity: 0.45,
  cursor: 'not-allowed',
};

const CONTENT_STYLE: CSSProperties = {
  background: 'var(--md-surface-container-highest)',
  borderRadius: 10,
  boxShadow: '0 4px 16px rgba(0,0,0,0.12)',
  maxHeight: 280,
  overflowY: 'auto',
  zIndex: 9999,
  border: '1px solid var(--md-outline-variant)',
};

const ITEM_STYLE: CSSProperties = {
  padding: '8px 12px',
  borderRadius: 6,
  cursor: 'pointer',
  fontSize: 14,
  color: 'var(--md-on-surface)',
  outline: 'none',
  display: 'flex',
  alignItems: 'center',
  userSelect: 'none',
};

const WRAPPER_STYLE: CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 6,
};

// ── 子组件 ──

function ChevronDown() {
  return (
    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--md-on-surface-variant)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="6 9 12 15 18 9" />
    </svg>
  );
}

function SelectLabel({ htmlFor, children }: { htmlFor?: string; children: ReactNode }) {
  return (
    <label
      htmlFor={htmlFor}
      style={{
        fontSize: 13,
        fontWeight: 600,
        color: 'var(--md-on-surface-variant)',
        letterSpacing: '0.01em',
      }}
    >
      {children}
    </label>
  );
}

function SelectErrorText({ children }: { children: ReactNode }) {
  return (
    <span style={{ fontSize: 12, color: 'var(--md-error)', marginTop: 4 }}>
      {children}
    </span>
  );
}

// ── 全局样式注入（Radix data-attribute 选择器） ──

const SELECT_STYLE_ID = 'inkforge-radix-select-styles';

function ensureSelectStylesInjected() {
  if (typeof document === 'undefined') return;
  if (document.getElementById(SELECT_STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = SELECT_STYLE_ID;
  style.textContent = `
    [data-radix-select-item][data-highlighted] {
      background: var(--md-surface-container-high);
    }
    [data-radix-select-item][data-state="checked"] {
      color: var(--md-primary);
      font-weight: 700;
      background: var(--md-surface-container-low);
    }
    .SelectTrigger[data-focus-visible] {
      border-color: var(--md-primary);
      box-shadow: 0 0 0 2px rgba(249, 115, 22, 0.18);
    }
  `;
  document.head.appendChild(style);
}

// ── 主组件 ──

export function Select({
  label,
  error,
  value,
  onChange,
  disabled,
  children,
  className,
  style,
  id: externalId,
}: SelectProps) {
  const generatedId = useId();
  const id = externalId ?? generatedId;
  const stringValue = value != null ? String(value) : undefined;

  // 在首次渲染时注入全局样式
  ensureSelectStylesInjected();

  return (
    <div style={{ ...WRAPPER_STYLE, ...style }} className={className}>
      {label && <SelectLabel htmlFor={id}>{label}</SelectLabel>}

      <RadixSelect.Root
        value={stringValue}
        disabled={disabled}
        onValueChange={(v) => onChange?.({ target: { value: v } })}
      >
        <RadixSelect.Trigger
          id={id}
          className="SelectTrigger"
          style={{
            ...TRIGGER_STYLE,
            ...(disabled ? TRIGGER_DISABLED_STYLE : {}),
            ...(error ? { borderColor: 'var(--md-error)' } : {}),
          }}
          aria-invalid={!!error}
        >
          <span
            style={{
              flex: 1,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            <RadixSelect.Value placeholder={'\u00A0'} />
          </span>
          <RadixSelect.Icon asChild>
            <span style={{ display: 'flex', alignItems: 'center', flexShrink: 0 }}>
              <ChevronDown />
            </span>
          </RadixSelect.Icon>
        </RadixSelect.Trigger>

        <RadixSelect.Portal>
          <RadixSelect.Content
            position="popper"
            sideOffset={4}
            style={CONTENT_STYLE}
          >
            <RadixSelect.ScrollUpButton />
            <RadixSelect.Viewport style={{ padding: 4 }}>
              {children}
            </RadixSelect.Viewport>
            <RadixSelect.ScrollDownButton />
          </RadixSelect.Content>
        </RadixSelect.Portal>
      </RadixSelect.Root>

      {error && <SelectErrorText>{error}</SelectErrorText>}
    </div>
  );
}

Select.displayName = 'Select';

// ── SelectItem 子组件 ──

export function SelectItem({ value, children }: { value: string; children: ReactNode }) {
  return (
    <RadixSelect.Item value={value} style={ITEM_STYLE}>
      <RadixSelect.ItemText>{children}</RadixSelect.ItemText>
    </RadixSelect.Item>
  );
}

SelectItem.displayName = 'SelectItem';

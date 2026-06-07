import { useI18n } from '../i18n';

/// 零状态时间选择器——完全由 props 驱动。
/// 以 Rust trait bound 的思维设计：改变行为 = 改变调用参数，不改组件代码。
interface TimePickerProps {
  hour: number;
  minute: number;
  onChange: (hour: number, minute: number) => void;
  minHour?: number;
  maxHour?: number;
  minuteStep?: number;
  disabled?: boolean;
}

const TIME_PICKER_HOURS_FROM_MIDNIGHT_TO_END_OF_DAY = Array.from({ length: 24 }, (_, i) => i);

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

  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
      <select
        value={hour}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value), minute)}
        style={SELECT_STYLE_FOR_TIMEPICKER_DROPDOWN}
      >
        {TIME_PICKER_HOURS_FROM_MIDNIGHT_TO_END_OF_DAY
          .filter((h) => h >= minHour && h <= maxHour)
          .map((h) => (
            <option key={h} value={h}>
              {String(h).padStart(2, '0')}
            </option>
          ))}
      </select>
      <span style={{ color: 'var(--md-outline)' }}>:</span>
      <select
        value={minute}
        disabled={disabled}
        onChange={(e) => onChange(hour, Number(e.target.value))}
        style={SELECT_STYLE_FOR_TIMEPICKER_DROPDOWN}
      >
        {availableMinutes.map((m) => (
          <option key={m} value={m}>
            {String(m).padStart(2, '0')}
          </option>
        ))}
      </select>
      <span style={{ fontSize: 12, color: 'var(--md-outline)' }}>{t('timeHourAbbreviation')}</span>
    </div>
  );
}

const SELECT_STYLE_FOR_TIMEPICKER_DROPDOWN: React.CSSProperties = {
  padding: '8px 12px',
  borderRadius: 10,
  border: '1px solid var(--md-outline-variant)',
  background: 'var(--md-surface-container-highest)',
  color: 'var(--md-on-surface)',
  fontSize: 14,
  fontFamily: 'inherit',
  outline: 'none',
};

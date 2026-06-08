import { Select, SelectItem } from './Select';
import { useI18n } from '../i18n';

// ── 零状态时间选择器 —— 完全由 props 驱动。 ──

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

// ── TimeUnitDropdown：基于 Radix Select 的时间单位下拉 ──

function TimeUnitDropdown({
  value,
  options,
  onChange,
  disabled,
}: {
  value: number;
  options: number[];
  onChange: (v: number) => void;
  disabled?: boolean;
}) {
  return (
    <Select
      value={String(value).padStart(2, "0")}
      disabled={disabled}
      onChange={(e) => onChange(Number(e.target.value))}
      style={{ width: 76 }}
    >
      {options.map((opt) => (
        <SelectItem key={opt} value={String(opt).padStart(2, "0")}>
          {String(opt).padStart(2, '0')}
        </SelectItem>
      ))}
    </Select>
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

  return (
    <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
      <TimeUnitDropdown
        value={hour}
        options={TIME_PICKER_HOURS_FROM_MIDNIGHT_TO_END_OF_DAY.filter((h) => h >= minHour && h <= maxHour)}
        onChange={(h) => onChange(h, minute)}
        disabled={disabled}
      />
      <span style={{ color: 'var(--md-outline)', fontSize: 14, fontWeight: 500 }}>:</span>
      <TimeUnitDropdown
        value={minute}
        options={availableMinutes}
        onChange={(m) => onChange(hour, m)}
        disabled={disabled}
      />
      <span style={{ fontSize: 12, color: 'var(--md-outline)' }}>{t('timeHourAbbreviation')}</span>
    </div>
  );
}

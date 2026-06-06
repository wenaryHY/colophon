import type { ReactNode } from 'react';

type StatsTheme = 'orange' | 'emerald' | 'amber' | 'blue' | 'rose' | 'violet';

/* ── MD3 container 色系：每个主题映射到 MD3 tonal pair ── */
const THEME_MAP: Record<StatsTheme, { iconBg: string; accent: string }> = {
  orange:  { iconBg: 'var(--md-primary-container)',     accent: 'var(--md-on-primary-container)' },
  emerald: { iconBg: 'var(--md-secondary-container)',    accent: 'var(--md-on-secondary-container)' },
  amber:   { iconBg: 'var(--md-primary-container)',      accent: 'var(--md-on-primary-container)' },
  blue:    { iconBg: 'var(--md-tertiary-container)',     accent: 'var(--md-on-tertiary-container)' },
  rose:    { iconBg: 'var(--md-error-container)',        accent: 'var(--md-on-error-container)' },
  violet:  { iconBg: 'var(--md-tertiary-container)',     accent: 'var(--md-on-tertiary-container)' },
};

interface StatsCardProps {
  icon: ReactNode;
  value: number | string;
  label: string;
  theme?: StatsTheme;
}

export function StatsCard({ icon, value, label, theme = 'orange' }: StatsCardProps) {
  const themeStyles = THEME_MAP[theme];

  return (
    <div style={{
      background: 'var(--md-surface-container-lowest)',
      borderRadius: 'var(--radius-lg)',
      padding: '20px 24px',
      transition: 'transform 0.2s var(--ease-default)',
      cursor: 'default',
    }}
    onMouseEnter={(e) => {
      e.currentTarget.style.transform = 'scale(0.98)';
    }}
    onMouseLeave={(e) => {
      e.currentTarget.style.transform = 'scale(1)';
    }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px' }}>
        <div style={{
          width: '40px', height: '40px', borderRadius: 'var(--radius-md)',
          background: themeStyles.iconBg,
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          color: themeStyles.accent,
        }}>{icon}</div>
      </div>
      <div style={{ fontSize: '24px', fontWeight: 800, color: 'var(--md-on-surface)', letterSpacing: '-0.5px', lineHeight: 1.15 }}>
        {value}
      </div>
      <div style={{ fontSize: '12px', fontWeight: 600, color: 'var(--md-outline)', marginTop: '4px', letterSpacing: '0.02em' }}>
        {label}
      </div>
    </div>
  );
}

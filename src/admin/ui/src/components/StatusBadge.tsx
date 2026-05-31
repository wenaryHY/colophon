import { useI18n } from '../i18n';

interface StatusBadgeProps {
  status: string;
}

/* ── MD3 chip 风格：语义色彩保持含义不变 ──
   绿色=成功/通过，黄色=警告/待处理，红色=危险/拒绝，灰色=禁用/已删除
*/
const STATUS_KEY_MAP: Record<string, string> = {
  published: 'statusPublished',
  draft: 'statusDraft',
  trashed: 'statusTrashed',
  pending: 'statusPending',
  approved: 'statusApproved',
  rejected: 'statusRejected',
  deleted: 'statusDeleted',
};

const COLOR_CONFIG: Record<string, { bg: string; dot: string; textColor: string }> = {
  published: { bg: 'var(--success-50)',  dot: 'var(--success-500)', textColor: 'var(--success-700)' },
  draft:     { bg: 'var(--warning-50)',  dot: 'var(--warning-500)', textColor: 'var(--warning-700)' },
  trashed:   { bg: 'var(--danger-50)',   dot: 'var(--danger-500)',  textColor: 'var(--danger-700)' },
  pending:   { bg: 'var(--warning-50)',  dot: 'var(--warning-500)', textColor: 'var(--warning-700)' },
  approved:  { bg: 'var(--success-50)',  dot: 'var(--success-500)', textColor: 'var(--success-700)' },
  rejected:  { bg: 'var(--danger-50)',   dot: 'var(--danger-500)',  textColor: 'var(--danger-700)' },
  deleted:   { bg: 'var(--md-surface-container)', dot: 'var(--md-outline)', textColor: 'var(--md-on-surface-variant)' },
};

export function StatusBadge({ status }: StatusBadgeProps) {
  const { t } = useI18n();
  const key = STATUS_KEY_MAP[status] || status;
  const label = t(key, status);
  const c = COLOR_CONFIG[status] || { bg: 'var(--md-surface-container)', dot: 'var(--md-outline)', textColor: 'var(--md-on-surface-variant)' };
  return (
    <span style={{
      display: 'inline-flex',
      alignItems: 'center',
      gap: '6px',
      padding: '4px 12px',
      minWidth: '72px',
      borderRadius: 'var(--radius-full)',
      fontSize: '12px',
      fontWeight: 600,
      background: c.bg,
      color: c.textColor,
      letterSpacing: '0.01em',
      whiteSpace: 'nowrap',
      justifyContent: 'center',
    }}>
      <span style={{
        width: '6px', height: '6px', borderRadius: '50%',
        background: c.dot,
        flexShrink: 0,
      }} />
      {label}
    </span>
  );
}

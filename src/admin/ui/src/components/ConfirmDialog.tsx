import { IconAlertCircle } from './Icons';
import { Modal } from './Modal';
import { Button } from './Button';
import { useI18n } from '../i18n';

interface ConfirmDialogProps {
  open: boolean;
  onClose: () => void;
  onConfirm: () => void;
  title?: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  variant?: 'danger' | 'warning' | 'info';
}

const BG_MAP = { danger: 'var(--danger-50)', warning: 'var(--warning-50)', info: 'var(--info-50)' };
const ICON_COLOR = { danger: 'var(--danger-500)', warning: 'var(--warning-500)', info: 'var(--info-500)' };
const BORDER_MAP = { danger: 'var(--danger-100)', warning: 'var(--warning-100)', info: 'var(--info-100)' };

export function ConfirmDialog({
  open, onClose, onConfirm,
  title, message,
  confirmText, cancelText,
  variant = 'danger',
}: ConfirmDialogProps) {
  const { t } = useI18n();
  const resolvedTitle = title || t('confirmDialogTitle');
  const resolvedConfirmText = confirmText || t('confirm');
  const resolvedCancelText = cancelText || t('cancel');
  return (
    <Modal
      open={open} onClose={onClose} title={resolvedTitle} width="420px"
      actions={
        <>
          <Button variant="ghost" onClick={onClose}>{resolvedCancelText}</Button>
          <Button variant={variant === 'danger' ? 'danger' : variant === 'warning' ? 'warning' : 'primary'}
            onClick={() => { onConfirm(); onClose(); }}>
            {resolvedConfirmText}
          </Button>
        </>
      }
    >
      {/* 设计指南：危险操作用语义色背景+图标强调区 */}
      <div style={{
        display: 'flex', alignItems: 'flex-start', gap: '12px',
        padding: '16px', borderRadius: 'var(--radius-md)',
        background: BG_MAP[variant],
        border: `1px solid ${BORDER_MAP[variant]}`,
      }}>
        <IconAlertCircle size={22} style={{ color: ICON_COLOR[variant], flexShrink: 0, marginTop: '1px' }} />
        <p style={{ fontSize: '14px', color: 'var(--text-secondary)', lineHeight: 1.6 }}>
          {message}
        </p>
      </div>
    </Modal>
  );
}

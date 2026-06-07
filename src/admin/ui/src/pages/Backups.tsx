import { useRef, useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import {
  API,
  API_PREFIX,
  createBackup,
  deleteBackup as deleteBackupApi,
  listBackups,
  mergeRestoreBackup,
  getBackupSchedule,
  updateBackupSchedule,
} from '../lib/api';
import { getQueryClient } from '../lib/api';
import type { BackupListResponse } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { Select } from '../components/Select';
import { TimePicker } from '../components/TimePicker';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';
import { useResponsive } from '../hooks/useResponsive';

// ── 样式常量（复用 Settings 的 Section/Card 模式） ──

const sectionStyle: React.CSSProperties = {
  background: 'var(--md-surface-container-lowest)',
  borderRadius: 'var(--radius-lg)',
  marginBottom: '20px',
};
const secHeadStyle: React.CSSProperties = {
  padding: '18px 24px', background: 'var(--md-surface-container-low)',
};
const secTitleStyle: React.CSSProperties = {
  fontSize: '15px', fontWeight: 700, color: 'var(--md-on-surface)', letterSpacing: '-0.2px',
};
const secDescStyle: React.CSSProperties = { fontSize: '12.5px', color: 'var(--md-outline)', marginTop: '3px' };
const secBodyStyle: React.CSSProperties = { padding: '24px', display: 'flex', flexDirection: 'column' as const, gap: '18px' };
const formRowStyle: React.CSSProperties = { display: 'grid', gridTemplateColumns: '160px 1fr', gap: '12px', alignItems: 'start' };
const formRowMobileStyle: React.CSSProperties = { display: 'flex', flexDirection: 'column' as const, gap: '6px' };
const labelStyle: React.CSSProperties = { fontSize: '13.5px', fontWeight: 600, color: 'var(--md-on-surface-variant)', paddingTop: '10px' };
const hintStyle: React.CSSProperties = { fontSize: '12px', color: 'var(--md-outline)', opacity: 0.8 };

function SettingSection({ title, description, children, isMobile: mobile }: { title: string; description?: string; children: React.ReactNode; isMobile?: boolean }) {
  if (mobile) {
    return (
      <details open style={sectionStyle}>
        <summary style={{
          ...secHeadStyle,
          cursor: 'pointer',
          listStyle: 'none',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          userSelect: 'none',
        }}>
          <div>
            <h3 style={secTitleStyle}>{title}</h3>
            {description && <p style={secDescStyle}>{description}</p>}
          </div>
          <span style={{ fontSize: '12px', color: 'var(--md-outline)', transition: 'transform 0.2s ease' }}>▼</span>
        </summary>
        <div style={secBodyStyle}>{children}</div>
      </details>
    );
  }

  return (
    <div style={sectionStyle}>
      <div style={secHeadStyle}>
        <h3 style={secTitleStyle}>{title}</h3>
        {description && <p style={secDescStyle}>{description}</p>}
      </div>
      <div style={secBodyStyle}>{children}</div>
    </div>
  );
}

function FormRow({ label, children, hint, isMobile: mobile }: { label: string; children: React.ReactNode; hint?: string; isMobile?: boolean }) {
  return (
    <div style={mobile ? formRowMobileStyle : formRowStyle}>
      <span style={labelStyle}>{label}</span>
      <div style={{ display: 'flex', flexDirection: 'column' as const, gap: '5px' }}>
        {children}
        {hint && <span style={hintStyle}>{hint}</span>}
      </div>
    </div>
  );
}

function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(2)} MB`;
}

// ── 可复用：inline toggle 开关 ──

const TOGGLE_TRACK_STYLE: React.CSSProperties = {
  width: '44px', height: '24px', borderRadius: '12px',
  border: 'none', cursor: 'pointer', position: 'relative',
  transition: 'background 0.2s ease', flexShrink: 0,
};

const TOGGLE_THUMB_STYLE: React.CSSProperties = {
  position: 'absolute', top: '2px',
  width: '20px', height: '20px', borderRadius: '50%',
  background: '#fff',
  transition: 'left 0.2s ease',
  boxShadow: '0 1px 3px rgba(0,0,0,0.2)',
};

function ToggleSwitch({ enabled, onChange, disabled }: { enabled: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={() => onChange(!enabled)}
      style={{
        ...TOGGLE_TRACK_STYLE,
        background: enabled ? 'var(--md-primary)' : 'var(--md-outline-variant)',
        opacity: disabled ? 0.5 : 1,
      }}
    >
      <span style={{ ...TOGGLE_THUMB_STYLE, left: enabled ? '22px' : '2px' }} />
    </button>
  );
}

// ── 格式化时间戳为本地字符串 ──

function formatDateTime(iso: string | null | undefined): string {
  if (!iso) return '—';
  try {
    return new Date(iso).toLocaleString('zh-CN');
  } catch {
    return iso;
  }
}

// ── 页面组件 ──

export default function Backups() {
  const toast = useToast();
  const { t, format } = useI18n();
  const { isMobile } = useResponsive();
  const restoreInputRef = useRef<HTMLInputElement>(null);
  const [downloadingBackupId, setDownloadingBackupId] = useState<string | null>(null);
  const [mergeRestoringId, setMergeRestoringId] = useState<string | null>(null);

  // ── 定时备份计划 ──
  const [scheduleEnabled, setScheduleEnabled] = useState(false);
  const [scheduleFrequency, setScheduleFrequency] = useState('daily');
  const [scheduleHour, setScheduleHour] = useState(3);
  const [scheduleMinute, setScheduleMinute] = useState(0);
  const [scheduleProvider, setScheduleProvider] = useState('local');
  const scheduleInitRef = useRef(false);

  const {
    data: scheduleData,
    isLoading: scheduleLoading,
  } = useQuery({
    queryKey: ['backup-schedule'],
    queryFn: () => getBackupSchedule(),
    staleTime: 30_000,
  });

  // 从接口数据初始化表单
  if (scheduleData && !scheduleInitRef.current) {
    setScheduleEnabled(scheduleData.enabled);
    setScheduleFrequency(scheduleData.frequency || 'daily');
    setScheduleHour(scheduleData.hour ?? 3);
    setScheduleMinute(scheduleData.minute ?? 0);
    setScheduleProvider(scheduleData.provider || 'local');
    scheduleInitRef.current = true;
  }

  const saveScheduleMutation = useMutation({
    mutationFn: () =>
      updateBackupSchedule({
        enabled: scheduleEnabled,
        frequency: scheduleFrequency,
        hour: scheduleHour,
        minute: scheduleMinute,
        provider: scheduleProvider,
      }),
    onSuccess: () => {
      toast(t('scheduleSaved'), 'success');
      getQueryClient().invalidateQueries({ queryKey: ['backup-schedule'] });
    },
    onError: (error) =>
      toast(error instanceof Error ? error.message : t('scheduleSaveFailed'), 'error'),
  });

  // ── 备份历史 ──
  const { data: backups = [], refetch: refetchBackups } = useQuery({
    queryKey: ['backups'],
    queryFn: () => listBackups(),
    staleTime: 10_000,
  });

  async function downloadBackupById(backupId: string) {
    try {
      setDownloadingBackupId(backupId);
      const res = await fetch(`${API}${API_PREFIX}/admin/backup/${backupId}`, {
        credentials: 'include',
      });
      if (!res.ok) {
        throw new Error(t('downloadBackupFailed'));
      }
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `inkforge_backup_${backupId}_${new Date().toISOString().slice(0, 10)}.zip`;
      a.click();
      URL.revokeObjectURL(url);
      toast(t('backupDownloadStarted'), 'success');
    } catch (error) {
      toast(error instanceof Error ? error.message : t('downloadBackupFailed'), 'error');
    } finally {
      setDownloadingBackupId(null);
    }
  }

  const createBackupMutation = useMutation({
    mutationFn: () => createBackup('local'),
    onSuccess: () => { toast(t('backupCreated'), 'success'); refetchBackups(); },
    onError: (error) => toast(error instanceof Error ? error.message : t('createBackupFailed'), 'error'),
  });

  const mergeRestoreMutation = useMutation({
    mutationFn: (backupId: string) => mergeRestoreBackup(backupId),
    onSuccess: () => {
      toast(t('mergeRestoreSuccess'), 'success');
      setMergeRestoringId(null);
      setTimeout(() => location.reload(), 1200);
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : t('mergeRestoreFailed'), 'error');
      setMergeRestoringId(null);
    },
  });

  const deleteBackupMutation = useMutation({
    mutationFn: (backupId: string) => deleteBackupApi(backupId),
    onSuccess: () => { toast(t('backupDeleted'), 'success'); refetchBackups(); },
    onError: (error) => toast(error instanceof Error ? error.message : t('deleteBackupFailed'), 'error'),
  });

  function handleMergeRestore(backupId: string) {
    if (!window.confirm(t('mergeRestoreConfirm'))) return;
    setMergeRestoringId(backupId);
    mergeRestoreMutation.mutate(backupId);
  }

  function handleDeleteBackup(backupId: string) {
    if (!window.confirm(t('deleteBackupConfirm'))) return;
    deleteBackupMutation.mutate(backupId);
  }

  return (
    <>
      <PageHeader title={t('title')} subtitle={t('subtitle')} />

      {/* ── Section 1: 定时备份 ── */}
      <SettingSection title={t('scheduledBackup')} description={t('scheduledBackupDesc')} isMobile={isMobile}>
        <FormRow label={t('enableScheduledBackup')} isMobile={isMobile}>
          <ToggleSwitch
            enabled={scheduleEnabled}
            onChange={setScheduleEnabled}
            disabled={scheduleLoading}
          />
        </FormRow>

        <FormRow label={t('backupFrequency')} isMobile={isMobile}>
          <Select
            value={scheduleFrequency}
            onChange={(e) => setScheduleFrequency(e.target.value)}
            disabled={!scheduleEnabled || scheduleLoading}
          >
            <option value="daily">{t('frequencyDaily')}</option>
            <option value="hourly">{t('frequencyHourly')}</option>
          </Select>
        </FormRow>

        <FormRow label={t('backupTime')} isMobile={isMobile}>
          <TimePicker
            hour={scheduleHour}
            minute={scheduleMinute}
            onChange={(h, m) => { setScheduleHour(h); setScheduleMinute(m); }}
            disabled={!scheduleEnabled || scheduleLoading}
          />
        </FormRow>

        <FormRow label={t('backupStorage')} isMobile={isMobile}>
          <Select
            value={scheduleProvider}
            onChange={(e) => setScheduleProvider(e.target.value)}
            disabled={!scheduleEnabled || scheduleLoading}
          >
            <option value="local">{t('providerLocal')}</option>
            <option value="s3">{t('providerS3')}</option>
          </Select>
        </FormRow>

        {/* 最近 / 下次执行时间 */}
        {scheduleData && scheduleEnabled && (
          <div style={{ display: 'flex', gap: '32px', fontSize: '12.5px', color: 'var(--md-outline)' }}>
            <span>{t('scheduleLastRun')}: {formatDateTime(scheduleData.last_run_at)}</span>
            <span>{t('scheduleNextRun')}: {formatDateTime(scheduleData.next_run_at)}</span>
          </div>
        )}

        <div>
          <Button
            onClick={() => saveScheduleMutation.mutate()}
            disabled={saveScheduleMutation.isPending || scheduleLoading}
            loading={saveScheduleMutation.isPending}
          >
            {t('saveSchedule')}
          </Button>
        </div>
      </SettingSection>

      {/* ── Section 2: 手动操作 ── */}
      <SettingSection title={t('manualBackup')} description={t('manualBackupDesc')} isMobile={isMobile}>
        <div style={{ display: 'flex', gap: '10px', alignItems: 'center', flexWrap: 'wrap' }}>
          <Button
            onClick={() => createBackupMutation.mutate()}
            disabled={createBackupMutation.isPending}
            loading={createBackupMutation.isPending}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M12 5v14"/><path d="M5 12h14"/></svg>
            {createBackupMutation.isPending ? t('creating') : t('createBackup')}
          </Button>
          <Button variant="ghost" onClick={() => restoreInputRef.current?.click()}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
            {t('importBackup')}
          </Button>
          <input
            ref={restoreInputRef}
            type="file"
            accept=".zip"
            style={{ display: 'none' }}
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (!file) return;
              if (!window.confirm(format('backupConfirm', { filename: file.name }))) {
                return;
              }
              const formData = new FormData();
              formData.append('file', file);
              fetch(`${API}${API_PREFIX}/admin/backup/restore`, {
                method: 'POST',
                body: formData,
                credentials: 'include',
              })
                .then((r) => r.json())
                .then((json) => {
                  if (json.code === 0) {
                    toast(t('backupImportSuccess'), 'success');
                    setTimeout(() => location.reload(), 1500);
                  } else {
                    toast(json.message || t('importFailed'), 'error');
                  }
                })
                .catch((err) => toast(err instanceof Error ? err.message : t('importFailed'), 'error'))
                .finally(() => {
                  if (restoreInputRef.current) restoreInputRef.current.value = '';
                });
            }}
          />
        </div>
      </SettingSection>

      {/* ── Section 3: 备份历史 ── */}
      <SettingSection title={t('backupHistory')} isMobile={isMobile}>
        <div style={{ paddingTop: '0' }}>
          <div style={{ fontSize: '11.5px', fontWeight: 700, color: 'var(--md-outline)', textTransform: 'uppercase' as const, letterSpacing: '0.08em', marginBottom: '12px' }}>
            {format('backupHistoryCount', { count: backups.length })}
          </div>
          {backups.length === 0 ? (
            <div style={{ fontSize: '13px', color: 'var(--md-outline)', padding: '20px 0', textAlign: 'center' }}>
              {t('noBackup')}
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {backups.map((b: BackupListResponse) => (
                <div
                  key={b.id}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    padding: '12px 16px',
                    borderRadius: 'var(--radius-md)',
                    background: 'var(--md-surface-container)',
                    transition: 'background 0.15s ease',
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.background = 'var(--md-surface-container-high)'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.background = 'var(--md-surface-container)'; }}
                >
                  <div style={{ display: 'flex', alignItems: 'center', gap: '14px', minWidth: 0 }}>
                    <div style={{
                      width: '34px', height: '34px', borderRadius: '8px',
                      background: b.status === 'completed' ? 'rgba(34,197,94,0.1)' : b.status === 'failed' ? 'rgba(239,68,68,0.1)' : 'rgba(250,204,21,0.1)',
                      display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '15px', flexShrink: 0,
                    }}>
                      {b.status === 'completed' ? '✅' : b.status === 'failed' ? '❌' : '⏳'}
                    </div>
                    <div style={{ minWidth: 0 }}>
                      <div style={{ fontSize: '13px', fontWeight: 600, color: 'var(--md-on-surface)', display: 'flex', gap: '8px', alignItems: 'center' }}>
                        <span style={{ fontFamily: 'monospace', fontSize: '12px', opacity: 0.7 }}>{b.id.slice(0, 8)}</span>
                        <span style={{
                          fontSize: '10.5px', fontWeight: 600, padding: '1px 6px', borderRadius: '4px',
                          background: b.provider === 's3' ? 'rgba(99,102,241,0.1)' : 'rgba(107,114,128,0.1)',
                          color: b.provider === 's3' ? '#6366f1' : '#6b7280',
                        }}>{b.provider}</span>
                      </div>
                      <div style={{ fontSize: '12px', color: 'var(--md-outline)', marginTop: '2px' }}>
                        {new Date(b.created_at).toLocaleString('zh-CN')} · {formatBytes(b.size)}
                        {b.error_message && <span style={{ color: '#ef4444', marginLeft: '8px' }}>({b.error_message})</span>}
                      </div>
                    </div>
                  </div>

                  <div style={{ display: 'flex', gap: '6px', flexShrink: 0 }}>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => downloadBackupById(b.id)}
                      disabled={downloadingBackupId === b.id}
                      loading={downloadingBackupId === b.id}
                      title={t('backupDownloadStarted')}
                    >
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleMergeRestore(b.id)}
                      disabled={mergeRestoringId === b.id || b.status !== 'completed'}
                      loading={mergeRestoringId === b.id}
                      title={t('backupMergeRestore')}
                    >
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M6 21V9a9 9 0 0 0 9 9"/></svg>
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleDeleteBackup(b.id)}
                      disabled={deleteBackupMutation.isPending}
                      loading={deleteBackupMutation.isPending}
                      title={t('deleteBackupConfirm')}
                      style={{ color: '#ef4444' }}
                    >
                      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </SettingSection>
    </>
  );
}

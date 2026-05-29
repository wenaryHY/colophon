import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import {
  apiData,
  API,
  API_PREFIX,
  createBackup,
  deleteBackup as deleteBackupApi,
  listBackups,
  mergeRestoreBackup,
} from '../lib/api';
import { getQueryClient } from '../lib/api';
import { SlotRenderer } from '../lib/slots';
import type { Setting, ThemeSummary } from '../types';
import { PageHeader } from '../components/PageHeader';
import { Button } from '../components/Button';
import { Input } from '../components/Input';
import { Select } from '../components/Select';
import { TimePicker } from '../components/TimePicker';
import { NumberWheelPicker } from '../components/NumberWheelPicker';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';
import { useAuth } from '../contexts/AuthContext';

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
const labelStyle: React.CSSProperties = { fontSize: '13.5px', fontWeight: 600, color: 'var(--md-on-surface-variant)', paddingTop: '10px' };
const hintStyle: React.CSSProperties = { fontSize: '12px', color: 'var(--md-outline)', opacity: 0.8 };
const preferenceGridStyle: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
  gap: '16px',
};
const preferenceCardStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: '16px',
  padding: '18px',
  borderRadius: 'var(--radius-lg)',
  background: 'var(--md-surface-container-low)',
};
const preferenceTitleStyle: React.CSSProperties = {
  fontSize: '15px',
  fontWeight: 700,
  color: 'var(--md-on-surface)',
  letterSpacing: '-0.02em',
};
const preferenceHintStyle: React.CSSProperties = {
  fontSize: '12.5px',
  lineHeight: 1.6,
  color: 'var(--md-on-surface-variant)',
};

function SettingSection({ title, description, children }: { title: string; description?: string; children: React.ReactNode }) {
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

function FormRow({ label, children, hint }: { label: string; children: React.ReactNode; hint?: string }) {
  return (
    <div style={formRowStyle}>
      <span style={labelStyle}>{label}</span>
      <div style={{ display: 'flex', flexDirection: 'column' as const, gap: '5px' }}>
        {children}
        {hint && <span style={hintStyle}>{hint}</span>}
      </div>
    </div>
  );
}

function PreferenceCard({ label, children, hint }: { label: string; children: React.ReactNode; hint?: string }) {
  return (
    <div style={preferenceCardStyle}>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
        <span style={preferenceTitleStyle}>{label}</span>
        {hint && <span style={preferenceHintStyle}>{hint}</span>}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
        {children}
      </div>
    </div>
  );
}

function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(2)} MB`;
}

export default function Settings() {
  const toast = useToast();
  const { t, lang, setLang } = useI18n();
  const { user, refreshUser } = useAuth();
  const restoreInputRef = useRef<HTMLInputElement>(null);
  const [kv, setKv] = useState<Record<string, string>>({});
  const [downloadingBackupId, setDownloadingBackupId] = useState<string | null>(null);
  const [mergeRestoringId, setMergeRestoringId] = useState<string | null>(null);

  // 加载设置和主题
  const { data: themes = [] } = useQuery({
    queryKey: ['settings-themes'],
    queryFn: () => apiData<ThemeSummary[]>(`${API_PREFIX}/admin/themes`),
    staleTime: 60_000,
  });

  const { data: settings = [] } = useQuery({
    queryKey: ['settings'],
    queryFn: () => apiData<Setting[]>(`${API_PREFIX}/admin/settings`),
    staleTime: 60_000,
  });

  // 从 query data 同步 kv 初始值（用 ref 确保只初始化一次）
  const kvInitRef = useRef(false);
  useEffect(() => {
    if (!kvInitRef.current && settings.length > 0) {
      const nextKv: Record<string, string> = {};
      settings.forEach((item) => { nextKv[item.key] = item.value; });
      setKv(nextKv);
      kvInitRef.current = true;
    }
  }, [settings]);

  // 加载备份
  const { data: backups = [], refetch: refetchBackups } = useQuery({
    queryKey: ['backups'],
    queryFn: () => listBackups(),
    staleTime: 10_000,
  });

  function update(key: string, value: string) { setKv((prev) => ({ ...prev, [key]: value })); }

  async function downloadBackupById(backupId: string) {
    try {
      setDownloadingBackupId(backupId);
      const res = await fetch(`${API}${API_PREFIX}/admin/backup/${backupId}`, {
        credentials: 'include',
      });

      if (!res.ok) {
        throw new Error('下载备份失败');
      }
      const blob = await res.blob();
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `inkforge_backup_${backupId}_${new Date().toISOString().slice(0, 10)}.zip`;
      a.click();
      URL.revokeObjectURL(url);
      toast('备份文件已开始下载', 'success');
    } catch (error) {
      toast(error instanceof Error ? error.message : '下载备份失败', 'error');
    } finally {
      setDownloadingBackupId(null);
    }
  }

  const createBackupMutation = useMutation({
    mutationFn: () => createBackup('local'),
    onSuccess: () => { toast('已创建新备份', 'success'); refetchBackups(); },
    onError: (error) => toast(error instanceof Error ? error.message : '创建备份失败', 'error'),
  });

  const mergeRestoreMutation = useMutation({
    mutationFn: (backupId: string) => mergeRestoreBackup(backupId),
    onSuccess: () => {
      toast('合并恢复成功，页面即将刷新', 'success');
      setMergeRestoringId(null);
      setTimeout(() => location.reload(), 1200);
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : '合并恢复失败', 'error');
      setMergeRestoringId(null);
    },
  });

  const deleteBackupMutation = useMutation({
    mutationFn: (backupId: string) => deleteBackupApi(backupId),
    onSuccess: () => { toast('备份已删除', 'success'); refetchBackups(); },
    onError: (error) => toast(error instanceof Error ? error.message : '删除备份失败', 'error'),
  });

  const saveSettingsMutation = useMutation({
    mutationFn: async () => {
      const payload: Record<string, string> = {};
      if (kv.site_title) payload.site_title = kv.site_title;
      if (kv.site_description) payload.site_description = kv.site_description;
      if (kv.site_url) payload.site_url = kv.site_url;
      payload.allow_register = String(kv.allow_register ?? true);
      payload.allow_comment = String(kv.allow_comment ?? true);
      payload.comment_require_login = String(kv.comment_require_login ?? true);
      payload.comment_moderation_mode = kv.comment_moderation_mode || 'all';
      payload.comment_max_length = String(kv.comment_max_length || 2000);
      payload.theme_default_mode = kv.theme_default_mode || 'system';

      await apiData(`${API_PREFIX}/admin/settings/batch`, {
        method: 'PATCH',
        body: JSON.stringify({ settings: payload }),
      });

      if (kv.active_theme) {
        await apiData(`${API_PREFIX}/admin/themes/${kv.active_theme}/activate`, { method: 'POST' });
      }
    },
    retry: 0,
    onSuccess: () => {
      toast('设置已保存', 'success');
      getQueryClient().invalidateQueries({ queryKey: ['settings'] });
      getQueryClient().invalidateQueries({ queryKey: ['settings-themes'] });
    },
    onError: (error) => toast(error instanceof Error ? error.message : '保存设置失败', 'error'),
  });

  const languageMutation = useMutation({
    mutationFn: async (newLang: 'zh' | 'en') => {
      setLang(newLang);
      if (user) {
        await apiData(`${API_PREFIX}/me/profile`, {
          method: 'PATCH',
          body: JSON.stringify({ language: newLang }),
        });
        await refreshUser();
      }
    },
    onError: (error) => toast(error instanceof Error ? error.message : '保存语言设置失败', 'error'),
  });

  function handleMergeRestore(backupId: string) {
    if (!window.confirm('将执行"合并恢复"：保留当前新数据并合并备份历史数据，是否继续？')) {
      return;
    }
    setMergeRestoringId(backupId);
    mergeRestoreMutation.mutate(backupId);
  }

  function handleDeleteBackup(backupId: string) {
    if (!window.confirm('确定删除这个备份吗？删除后不可恢复。')) return;
    deleteBackupMutation.mutate(backupId);
  }

  const activeThemeOptions = useMemo(() => themes.map((t) => ({ value: t.manifest.slug, label: t.manifest.name })), [themes]);

  return (
    <>
      <PageHeader title={t('title')} subtitle={t('subtitle')}
        actions={<Button onClick={() => saveSettingsMutation.mutate()} disabled={saveSettingsMutation.isPending} loading={saveSettingsMutation.isPending}>{t('saveChanges')}</Button>} />

      <SettingSection title="基础信息" description="站点名称、描述等核心信息">
        <FormRow label="站点标题">
          <Input value={kv.site_title || ''} onChange={(e) => update('site_title', e.target.value)} placeholder="InkForge" />
        </FormRow>
        <FormRow label="站点描述" hint="用于 SEO 和页面 meta 描述，建议不超过 160 字符">
          <Input value={kv.site_description || ''} onChange={(e) => update('site_description', e.target.value)} placeholder="A personal blog powered by InkForge" />
        </FormRow>
        <FormRow label="站点 URL" hint="博客的完整访问地址，必须是纯 origin，例如 https://example.com">
          <Input value={kv.site_url || ''} onChange={(e) => update('site_url', e.target.value)} placeholder="https://example.com" />
        </FormRow>
        <FormRow label="后台 URL" hint="admin_url 由 site_url 自动推导，修改 site_url 即可同步更新">
          <Input value={kv.admin_url || ''} disabled placeholder="https://example.com/admin" />
        </FormRow>
      </SettingSection>

      <SettingSection title="评论与注册" description="控制用户交互和内容审核策略">
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '32px' }}>
          <div style={{ display: 'flex', flexDirection: 'column' as const, gap: '18px' }}>
            <FormRow label="公开注册">
              <Select value={kv.allow_register || 'true'} onChange={(e) => update('allow_register', e.target.value)}>
                <option value="true">允许新用户注册</option><option value="false">关闭注册</option>
              </Select>
            </FormRow>
            <FormRow label="允许评论">
              <Select value={kv.allow_comment || 'true'} onChange={(e) => update('allow_comment', e.target.value)}>
                <option value="true">允许评论</option><option value="false">全局关闭评论</option>
              </Select>
            </FormRow>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column' as const, gap: '18px' }}>
            <FormRow label="评论需登录">
              <Select value={kv.comment_require_login || 'true'} onChange={(e) => update('comment_require_login', e.target.value)}>
                <option value="true">是 — 仅登录可评论</option><option value="false">否 — 游客也可评论</option>
              </Select>
            </FormRow>
            <FormRow label="审核策略">
              <Select value={kv.comment_moderation_mode || 'all'} onChange={(e) => update('comment_moderation_mode', e.target.value)}>
                <option value="all">全部待审</option><option value="first_comment">首条待审，后续放行</option><option value="none">无需审核，直接发布</option>
              </Select>
            </FormRow>
          </div>
        </div>
        <FormRow label="评论最大长度" hint="单条评论允许的最大字符数">
          <Input type="number" value={kv.comment_max_length || '2000'} onChange={(e) => update('comment_max_length', e.target.value)} />
        </FormRow>
      </SettingSection>

      <SettingSection title="主题与外观" description="切换和管理已安装的前台主题">
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '32px' }}>
          <div style={{ display: 'flex', flexDirection: 'column' as const, gap: '18px' }}>
            <FormRow label="当前主题">
              <Select value={kv.active_theme || activeThemeOptions[0]?.value || 'default'}
                onChange={(e) => update('active_theme', e.target.value)}>
                {activeThemeOptions.map((o) => <option key={o.value} value={o.value}>{o.label}</option>)}
              </Select>
            </FormRow>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column' as const, gap: '18px' }}>
            <FormRow label="默认模式">
              <Select value={kv.theme_default_mode || 'system'} onChange={(e) => update('theme_default_mode', e.target.value)}>
                <option value="system">跟随系统</option><option value="light">浅色模式</option><option value="dark">深色模式</option>
              </Select>
            </FormRow>
          </div>
        </div>

        <div style={{ marginTop: '22px', paddingTop: '18px', background: 'var(--md-surface-container-low)', borderRadius: 'var(--radius-md)', padding: '18px' }}>
          <div style={{ fontSize: '11.5px', fontWeight: 700, color: 'var(--md-outline)', textTransform: 'uppercase' as const, letterSpacing: '0.08em', marginBottom: '16px' }}>
            已安装的主题
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))', gap: '14px' }}>
            {themes.map((theme) => (
              <div
                key={theme.manifest.slug}
                style={{
                  borderRadius: 'var(--radius-lg)',
                  padding: '18px', transition: 'transform 0.2s ease',
                  background: theme.active ? 'var(--md-primary-container)' : 'var(--md-surface-container)',
                  position: 'relative',
                }}
                onMouseEnter={(e) => {
                  if (!theme.active) { e.currentTarget.style.transform = 'scale(0.97)'; }
                }}
                onMouseLeave={(e) => {
                  if (!theme.active) { e.currentTarget.style.transform = 'scale(1)'; }
                }}
              >
                {theme.active && (
                  <span style={{
                    position: 'absolute', top: '-9px', right: '16px',
                    background: 'var(--md-primary)',
                    color: 'var(--md-on-primary)', fontSize: '10px', fontWeight: 700,
                    padding: '3px 10px', borderRadius: 'var(--radius-full)', letterSpacing: '0.06em',
                    textTransform: 'uppercase',
                  }}>使用中</span>
                )}
                <div style={{ display: 'flex', alignItems: 'center', gap: '12px', marginBottom: '12px' }}>
                  <div style={{
                    width: '40px', height: '40px', borderRadius: '11px',
                    background: theme.active ? 'var(--md-primary-container)' : 'var(--md-surface-container)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '17px',
                  }}>📄</div>
                  <div>
                    <div style={{ fontSize: '15px', fontWeight: 700, color: theme.active ? 'var(--md-primary)' : 'var(--md-on-surface)' }}>{theme.manifest.name}</div>
                    <div style={{ fontSize: '11px', color: 'var(--md-outline)', fontFamily: 'monospace', marginTop: '1px' }}>v{theme.manifest.version} · {theme.manifest.slug}</div>
                  </div>
                </div>
                <p style={{ fontSize: '13px', color: 'var(--md-on-surface-variant)', lineHeight: 1.6, display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' as const, overflow: 'hidden' }}>{theme.manifest.description}</p>
                <div style={{ marginTop: '14px', paddingTop: '13px', background: 'var(--md-surface-container-low)', borderRadius: 'var(--radius-sm)', padding: '8px 10px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span style={{ fontSize: '11.5px', color: 'var(--md-outline)' }}>作者：{theme.manifest.author}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
      </SettingSection>

      <SettingSection title="回收站与清理" description="配置已删除内容的保留天数与自动清理时间">
        <div style={preferenceGridStyle}>
          <PreferenceCard label="保留天数" hint="软删除的内容将被保存的天数，最长 90 天。过期后自动永久清理。">
            <NumberWheelPicker
              value={parseInt(kv.trash_retention_days || '30')}
              min={1}
              max={90}
              suffix="天"
              onChange={(val) => update('trash_retention_days', val.toString())}
            />
          </PreferenceCard>

          <PreferenceCard label="自动清理时间" hint="每天执行自动永久清理任务的时间。建议设在凌晨，避开访问高峰。">
            <TimePicker
              hour={parseInt(kv.trash_cleanup_hour || '3')}
              minute={parseInt(kv.trash_cleanup_minute || '0')}
              onChange={(h, m) => {
                update('trash_cleanup_hour', h.toString());
                update('trash_cleanup_minute', m.toString());
              }}
            />
          </PreferenceCard>
        </div>
      </SettingSection>

      <SettingSection title={t('uiSettings')} description={t('uiSettingsDesc')}>
        <FormRow label={t('interfaceLanguage')}>
          <Select value={lang} onChange={(e) => languageMutation.mutate(e.target.value as 'zh' | 'en')}>
            <option value="zh">{t('languageZh')}</option>
            <option value="en">{t('languageEn')}</option>
          </Select>
        </FormRow>
      </SettingSection>

      <SettingSection
        title={t('dataBackup')}
        description={t('dataBackupDesc')}
      >
        <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
          <div style={{ display: 'flex', gap: '10px', alignItems: 'center', flexWrap: 'wrap' }}>
            <Button onClick={() => createBackupMutation.mutate()} disabled={createBackupMutation.isPending} loading={createBackupMutation.isPending}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M12 5v14"/><path d="M5 12h14"/></svg>
              {createBackupMutation.isPending ? '创建中…' : '创建备份'}
            </Button>
            <Button variant="ghost" onClick={() => restoreInputRef.current?.click()}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>
              导入备份文件
            </Button>
            <input
              ref={restoreInputRef}
              type="file"
              accept=".zip"
              style={{ display: 'none' }}
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (!file) return;
                if (!window.confirm(`即将用 "${file.name}" 替换当前数据库，原数据库会备份为 .bak 文件。是否继续？`)) {
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
                      toast('备份导入成功，页面将刷新...', 'success');
                      setTimeout(() => location.reload(), 1500);
                    } else {
                      toast(json.message || '导入失败', 'error');
                    }
                  })
                  .catch((err) => toast(err instanceof Error ? err.message : '导入失败', 'error'))
                  .finally(() => {
                    if (restoreInputRef.current) restoreInputRef.current.value = '';
                  });
              }}
            />
          </div>

          <div style={{ paddingTop: '16px' }}>
            <div style={{ fontSize: '11.5px', fontWeight: 700, color: 'var(--md-outline)', textTransform: 'uppercase' as const, letterSpacing: '0.08em', marginBottom: '12px' }}>
              备份历史 ({backups.length})
            </div>
            {backups.length === 0 ? (
              <div style={{ fontSize: '13px', color: 'var(--md-outline)', padding: '20px 0', textAlign: 'center' }}>
                暂无备份记录，点击上方「创建备份」生成第一份
              </div>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                {backups.map((b) => (
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
                        title="下载此备份"
                      >
                        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleMergeRestore(b.id)}
                        disabled={mergeRestoringId === b.id || b.status !== 'completed'}
                        loading={mergeRestoringId === b.id}
                        title="合并恢复：保留当前新数据，合并此备份的历史数据"
                      >
                        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><circle cx="18" cy="18" r="3"/><circle cx="6" cy="6" r="3"/><path d="M6 21V9a9 9 0 0 0 9 9"/></svg>
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleDeleteBackup(b.id)}
                        disabled={deleteBackupMutation.isPending}
                        loading={deleteBackupMutation.isPending}
                        title="删除此备份"
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
        </div>
      </SettingSection>

      <SlotRenderer target="settings.sub_section" />
    </>
  );
}

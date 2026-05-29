import { useMemo, useState } from 'react';
import { useQuery, useMutation } from '@tanstack/react-query';
import { apiData, API_PREFIX } from '../lib/api';

import type { SetupInitializeResponse, SetupStatusResponse } from '../types';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';

// Tauri 环境检测与窗口跳转
function isTauriEnv(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function navigateToAdmin(target: string): Promise<void> {
  if (isTauriEnv()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('switch_to_admin');
      return;
    } catch (e) {
      console.warn('Tauri switch_to_admin failed, falling back to location redirect', e);
    }
  }
  window.location.href = target;
}

interface FormState {
  site_title: string;
  site_description: string;
  site_url: string;
  admin_url: string;
  allow_register: boolean;
  username: string;
  email: string;
  password: string;
  display_name: string;
}

function deriveAdminUrl(siteUrl: string): string {
  const trimmed = siteUrl.trim();
  if (!trimmed) return '';
  try {
    const parsed = new URL(trimmed);
    return `${parsed.origin}/admin`;
  } catch {
    return '';
  }
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="md3-field">
      <span className="md3-label">{label}</span>
      {children}
      {hint ? <span className="md3-hint">{hint}</span> : null}
    </label>
  );
}

export default function Setup() {
  const toast = useToast();
  const { t, lang, setLang } = useI18n();
  const [submitting, setSubmitting] = useState(false);
  const [completed, setCompleted] = useState(false);
  const [redirectTarget, setRedirectTarget] = useState('/admin');
  const [form, setForm] = useState<FormState>({
    site_title: 'InkForge',
    site_description: '',
    site_url: '',
    admin_url: '',
    allow_register: true,
    username: '',
    email: '',
    password: '',
    display_name: '',
  });

  const { isLoading } = useQuery({
    queryKey: ['setup-status'],
    queryFn: async () => {
      const status = await apiData<SetupStatusResponse>(`${API_PREFIX}/setup/status`);
      if (status.installed) {
        window.location.replace(status.admin_url || '/admin');
        return status;
      }
      setForm((prev) => ({
        ...prev,
        site_title: status.site_title || prev.site_title,
        site_description: status.site_description || '',
        site_url: status.site_url || '',
        admin_url: status.admin_url || deriveAdminUrl(status.site_url || ''),
        allow_register: status.allow_register,
      }));
      return status;
    },
    staleTime: 30_000,
    retry: false,
  });

  const initializeMutation = useMutation({
    mutationFn: () => apiData<SetupInitializeResponse>(`${API_PREFIX}/setup/initialize`, {
      method: 'POST',
      body: JSON.stringify(form),
    }),
    onSuccess: (payload) => {
      const nextTarget = payload.redirect_to || '/admin';
      setRedirectTarget(nextTarget);
      setCompleted(true);
      toast('安装完成，正在进入后台…', 'success');
      window.setTimeout(() => {
        navigateToAdmin(nextTarget);
      }, 900);
      setSubmitting(false);
    },
    onError: (error) => {
      toast(error instanceof Error ? error.message : '安装失败', 'error');
      setSubmitting(false);
    },
  });

  const canSubmit = useMemo(() => {
    return Boolean(
      form.site_title.trim() &&
      form.site_url.trim() &&
      form.admin_url.trim() &&
      form.username.trim() &&
      form.email.trim() &&
      form.password.trim(),
    );
  }, [form]);

  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    setForm((prev) => ({ ...prev, [key]: value }));
  }

  function handleSiteUrlChange(value: string) {
    setForm((prev) => {
      const nextSiteUrl = value;
      const derivedCurrent = deriveAdminUrl(prev.site_url);
      const shouldFollow = !prev.admin_url || prev.admin_url === derivedCurrent;
      return {
        ...prev,
        site_url: nextSiteUrl,
        admin_url: shouldFollow ? deriveAdminUrl(nextSiteUrl) : prev.admin_url,
      };
    });
  }

  function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    if (!canSubmit || submitting) return;
    setSubmitting(true);
    initializeMutation.mutate();
  }

  if (isLoading) {
    return (
      <div className="setup-wrapper">
        <div className="setup-loading">{t('checkingSetup')}</div>
      </div>
    );
  }

  if (completed) {
    return (
      <div className="setup-wrapper">
        <div className="md3-card md3-card-completed if-scale-in">
          <div className="md3-card-body">
            <div className="md3-icon-success">✓</div>
            <h2 className="md3-title font-headline">{t('setupCompleted')}</h2>
            <p className="md3-desc">{t('setupCompletedDesc')}</p>
            <div className="md3-target-url">{redirectTarget}</div>
            <button
              type="button"
              className="md3-btn md3-btn-primary"
              onClick={() => { navigateToAdmin(redirectTarget); }}
            >
              {t('enterAdmin')}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <>
      <style>{`
        .setup-wrapper {
          min-height: 100vh;
          background: linear-gradient(180deg, rgba(249,115,22,0.08) 0%, rgba(255,255,255,0.92) 30%, var(--md-background) 100%);
          padding: 32px 20px 48px;
          display: flex;
          justify-content: center;
          align-items: center;
          position: relative;
          overflow: hidden;
        }
        .setup-bg-decor1 {
          position: absolute; top: -120px; left: -90px; width: 360px; height: 360px; border-radius: 50%;
          background: radial-gradient(circle, rgba(249,115,22,0.15) 0%, transparent 72%);
        }
        .setup-bg-decor2 {
          position: absolute; bottom: -120px; right: -120px; width: 420px; height: 420px; border-radius: 50%;
          background: radial-gradient(circle, rgba(251,191,36,0.12) 0%, transparent 74%);
        }
        
        .md3-card {
          width: min(960px, 100%);
          background: rgba(255,255,255,0.85);
          border-radius: var(--radius-lg);
          box-shadow: 0 12px 48px rgba(15, 23, 42, 0.08);
          backdrop-filter: blur(24px);
          -webkit-backdrop-filter: blur(24px);
          border: 1px solid rgba(255,255,255,0.6);
          overflow: hidden;
          position: relative;
          z-index: 1;
          transition: transform var(--transition-smooth), box-shadow var(--transition-smooth);
        }
        .md3-card:hover {
          transform: translateY(-2px);
          box-shadow: 0 24px 64px rgba(15, 23, 42, 0.12);
        }
        .md3-card-completed {
          max-width: 560px;
        }
        
        .md3-header {
          padding: 40px 36px 34px;
          background: linear-gradient(135deg, var(--md-primary-container) 0%, #ffe8cc 100%);
          color: var(--md-on-primary-container);
        }
        .md3-header-top {
          display: flex; justify-content: space-between; gap: 16px; align-items: flex-start; flex-wrap: wrap;
        }
        .md3-badge {
          display: inline-flex; align-items: center; gap: 8px; padding: 6px 12px;
          border-radius: var(--radius-full); background: rgba(255,255,255,0.5);
          font-size: 12px; font-weight: 700; margin-bottom: 16px;
          color: var(--md-on-primary-container);
        }
        .md3-header-title {
          margin: 0; font-size: 32px; line-height: 1.2; font-weight: 800;
        }
        .md3-header-desc {
          margin: 10px 0 0; max-width: 520px; font-size: 14px; line-height: 1.75;
          color: rgba(124, 45, 18, 0.85);
        }
        
        .md3-lang-switcher {
          display: flex; gap: 4px; background: rgba(255,255,255,0.4);
          padding: 4px; border-radius: var(--radius-full);
        }
        .md3-lang-btn {
          border: none; border-radius: var(--radius-full); padding: 6px 14px;
          font-size: 12px; font-weight: 700; cursor: pointer;
          transition: all var(--transition-normal);
        }
        .md3-lang-btn.active {
          background: #fff; color: var(--md-primary);
          box-shadow: 0 2px 8px rgba(0,0,0,0.05);
        }
        .md3-lang-btn:not(.active) {
          background: transparent; color: var(--md-on-primary-container);
        }
        .md3-lang-btn:not(.active):hover {
          background: rgba(255,255,255,0.2);
        }
        
        .md3-form {
          padding: 30px 32px 34px; display: grid; gap: 32px;
        }
        .md3-section {
          display: grid; gap: 20px;
        }
        .md3-section-title {
          margin: 0; font-size: 18px; font-weight: 800; color: var(--md-on-surface);
        }
        .md3-section-desc {
          margin: 6px 0 0; font-size: 13px; color: var(--md-on-surface-variant); line-height: 1.7;
        }
        .md3-grid-2 {
          display: grid; gap: 20px; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
        }
        .md3-grid-3 {
          display: grid; gap: 20px; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
        }
        
        .md3-field {
          display: flex; flex-direction: column; gap: 8px;
        }
        .md3-label {
          font-size: 13px; font-weight: 700; color: var(--md-on-surface);
        }
        .md3-hint {
          font-size: 12px; color: var(--md-outline);
        }
        .md3-input {
          width: 100%; padding: 13px 16px;
          border-radius: var(--radius-md);
          border: 1px solid var(--md-outline-variant);
          background: var(--md-surface-container-lowest);
          font-size: 14px; color: var(--md-on-surface);
          outline: none; box-sizing: border-box;
          transition: all var(--transition-normal);
          font-family: inherit;
        }
        .md3-input:hover:not(:focus) {
          border-color: var(--md-outline);
          background: var(--md-surface-container-low);
        }
        .md3-input:focus {
          border-color: var(--md-primary);
          box-shadow: 0 0 0 4px var(--md-primary-container);
          background: #fff;
        }
        select.md3-input {
          appearance: none;
          background-image: url('data:image/svg+xml;utf8,<svg fill="%23777b84" height="24" viewBox="0 0 24 24" width="24" xmlns="http://www.w3.org/2000/svg"><path d="M7 10l5 5 5-5z"/></svg>');
          background-repeat: no-repeat;
          background-position-x: calc(100% - 12px);
          background-position-y: center;
        }
        
        .md3-btn-primary {
          min-width: 190px; border: none; border-radius: var(--radius-full);
          background: var(--md-primary); color: var(--md-on-primary);
          font-size: 14px; font-weight: 800; padding: 14px 24px;
          cursor: pointer; position: relative; overflow: hidden;
          box-shadow: 0 4px 12px rgba(249,115,22,0.2);
          transition: all var(--transition-normal);
        }
        .md3-btn-primary:not(:disabled):hover {
          transform: translateY(-2px);
          box-shadow: 0 8px 24px rgba(249,115,22,0.3);
          background: linear-gradient(0deg, rgba(255,255,255,0.1), rgba(255,255,255,0.1)), var(--md-primary);
        }
        .md3-btn-primary:not(:disabled):active {
          transform: scale(0.98) translateY(0);
          box-shadow: 0 2px 8px rgba(249,115,22,0.2);
        }
        .md3-btn-primary:disabled {
          cursor: not-allowed; opacity: 0.6; box-shadow: none;
        }
        
        .md3-footer {
          display: flex; justify-content: space-between; gap: 16px; align-items: center; flex-wrap: wrap;
          padding-top: 12px;
          border-top: 1px solid var(--md-surface-container-high);
        }
        .md3-footer-text {
          font-size: 13px; color: var(--md-on-surface-variant); line-height: 1.7; flex: 1;
        }
        
        .md3-card-body {
          padding: 48px 42px; text-align: center;
        }
        .md3-icon-success {
          width: 72px; height: 72px; margin: 0 auto 24px; border-radius: var(--radius-xl);
          display: grid; place-items: center;
          background: var(--md-primary-container); color: var(--md-on-primary-container);
          font-size: 32px; font-weight: 800;
          box-shadow: 0 8px 24px rgba(249,115,22,0.15);
        }
        .md3-title {
          margin: 0; font-size: 28px; font-weight: 800; color: var(--md-on-surface);
        }
        .md3-desc {
          margin: 12px 0 0; font-size: 15px; line-height: 1.75; color: var(--md-on-surface-variant);
        }
        .md3-target-url {
          margin: 24px 0; font-size: 14px; color: var(--md-on-surface); word-break: break-all;
          padding: 12px 20px; background: var(--md-surface-container-low); border-radius: var(--radius-md);
          border: 1px solid var(--md-outline-variant); font-family: monospace;
        }
        
        .setup-loading {
          font-size: 15px; color: var(--md-outline); font-weight: 600;
        }
      `}</style>
      
      <div className="setup-wrapper">
        <div style={{ position: 'absolute', inset: 0, pointerEvents: 'none' }}>
          <div className="setup-bg-decor1" />
          <div className="setup-bg-decor2" />
        </div>

        <div className="md3-card if-slide-up">
          <div className="md3-header">
            <div className="md3-header-top">
              <div>
                <div className="md3-badge">
                  {t('setupBadge')}
                </div>
                <h1 className="md3-header-title font-headline">{t('setupTitle')}</h1>
                <p className="md3-header-desc">
                  {t('setupDesc')}
                </p>
              </div>
              <div className="md3-lang-switcher">
                {(['zh', 'en'] as const).map((value) => (
                  <button
                    key={value}
                    type="button"
                    onClick={() => setLang(value)}
                    className={`md3-lang-btn ${lang === value ? 'active' : ''}`}
                  >
                    {value === 'zh' ? t('langZh') : t('langEn')}
                  </button>
                ))}
              </div>
            </div>
          </div>

          <form onSubmit={handleSubmit} className="md3-form">
            <section className="md3-section">
              <div>
                <h2 className="md3-section-title font-headline">{t('siteSection')}</h2>
                <p className="md3-section-desc">{t('siteSectionDesc')}</p>
              </div>
              <div className="md3-grid-2">
                <Field label={t('siteTitle')}>
                  <input value={form.site_title} onChange={(e) => update('site_title', e.target.value)} className="md3-input" placeholder="InkForge" />
                </Field>
                <Field label={t('siteUrl')} hint={t('siteUrlHint')}>
                  <input value={form.site_url} onChange={(e) => handleSiteUrlChange(e.target.value)} className="md3-input" placeholder="http://localhost:2000" />
                </Field>
                <Field label={t('adminUrl')} hint={t('adminUrlHint')}>
                  <input value={form.admin_url} onChange={(e) => update('admin_url', e.target.value)} className="md3-input" placeholder="http://localhost:2000/admin" />

                </Field>
                <Field label={t('allowRegister')}>
                  <select value={form.allow_register ? 'true' : 'false'} onChange={(e) => update('allow_register', e.target.value === 'true')} className="md3-input">
                    <option value="true">{t('allowRegisterOn')}</option>
                    <option value="false">{t('allowRegisterOff')}</option>
                  </select>
                </Field>
              </div>
              <Field label={t('siteDescription')} hint={t('siteDescriptionHint')}>
                <textarea value={form.site_description} onChange={(e) => update('site_description', e.target.value)} className="md3-input" style={{ minHeight: '108px', resize: 'vertical' }} placeholder="A personal blog powered by InkForge" />
              </Field>
            </section>

            <section className="md3-section">
              <div>
                <h2 className="md3-section-title font-headline">{t('adminSection')}</h2>
                <p className="md3-section-desc">{t('adminSectionDesc')}</p>
              </div>
              <div className="md3-grid-3">
                <Field label={t('username')}>
                  <input value={form.username} onChange={(e) => update('username', e.target.value)} className="md3-input" placeholder="admin" />
                </Field>
                <Field label={t('email')}>
                  <input type="email" value={form.email} onChange={(e) => update('email', e.target.value)} className="md3-input" placeholder="admin@example.com" />
                </Field>
                <Field label={t('displayName')} hint={t('displayNameHint')}>
                  <input value={form.display_name} onChange={(e) => update('display_name', e.target.value)} className="md3-input" placeholder="管理员" />
                </Field>
                <Field label={t('password')} hint={t('passwordHint')}>
                  <input type="password" value={form.password} onChange={(e) => update('password', e.target.value)} className="md3-input" placeholder="******" />
                </Field>
              </div>
            </section>

            <div className="md3-footer">
              <span className="md3-footer-text">
                {t('footerText')}
              </span>
              <button
                type="submit"
                disabled={!canSubmit || submitting}
                className="md3-btn-primary"
              >
                {submitting ? t('installing') : t('submitBtn')}
              </button>
            </div>
          </form>
        </div>
      </div>
    </>
  );
}

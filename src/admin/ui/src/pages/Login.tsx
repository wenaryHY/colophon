import { useEffect, useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { apiData, API_PREFIX } from '../lib/api';
import { SlotRenderer } from '../lib/slots';
import type { SetupStatusResponse } from '../types';
import { useAuth } from '../contexts/AuthContext';
import { useToast } from '../contexts/ToastContext';
import { useI18n } from '../i18n';
import { useNavigate } from 'react-router-dom';
import { useResponsive } from '../hooks/useResponsive';

type Tab = 'login' | 'register';

export default function Login() {
  const [tab, setTab] = useState<Tab>('login');
  const { login, register } = useAuth();
  const toast = useToast();
  const { t, lang, setLang } = useI18n();
  const { isMobile } = useResponsive();

  const [loginValue, setLoginValue] = useState('');
  const [loginPassword, setLoginPassword] = useState('');
  const [loginLoading, setLoginLoading] = useState(false);
  const [rememberMe, setRememberMe] = useState(false);

  const [regUsername, setRegUsername] = useState('');
  const [regEmail, setRegEmail] = useState('');
  const [regPassword, setRegPassword] = useState('');
  const [regDisplayName, setRegDisplayName] = useState('');

  // Turnstile widget token
  const turnstileRef = useRef<string | null>(null);
  const [turnstileReady, setTurnstileReady] = useState(false);

  useEffect(() => {
    // Turnstile 脚本已在 index.html 中通过 <script> 标签加载（render=explicit 模式）
    const interval = setInterval(() => {
      try {
        const t = (window as unknown as { turnstile?: { render: (selector: string, opts: Record<string, unknown>) => void } }).turnstile;
        if (t && document.getElementById('turnstile-widget')) {
          t.render('#turnstile-widget', {
            sitekey: '0x4AAAAAADffbuvTrWkvKyda',
            theme: 'dark',
            callback: (token: string) => {
              turnstileRef.current = token;
              setTurnstileReady(true);
            },
            'error-callback': () => {
              turnstileRef.current = null;
              setTurnstileReady(false);
            },
          });
          clearInterval(interval);
        }
      } catch {
        // 脚本尚未加载，等待
      }
    }, 200);
    return () => clearInterval(interval);
  }, []);

  const { data: setupStatus, isLoading: setupLoading } = useQuery({
    queryKey: ['setup-status'],
    queryFn: () => apiData<SetupStatusResponse>(`${API_PREFIX}/setup/status`),
    staleTime: 30_000,
    retry: false,
  });

  const registerAvailable = setupStatus?.allow_register ?? false;

  const navigate = useNavigate();

  useEffect(() => {
    if (setupStatus && !setupStatus.installed) {
      navigate('/setup');
    }
  }, [setupStatus, navigate]);

  const tabs = useMemo<Tab[]>(
    () => (registerAvailable ? ['login', 'register'] : ['login']),
    [registerAvailable],
  );
  const activeTabIndex = Math.max(0, tabs.indexOf(tab));
  const showTabs = tabs.length > 1;

  async function handleLogin(e: React.FormEvent) {
    e.preventDefault();
    if (!loginValue || !loginPassword) return;
    setLoginLoading(true);
    try {
      const result = await login(loginValue, loginPassword, rememberMe, turnstileRef.current);
      if (result.success) {
        setTimeout(() => navigate('/posts'), 100);
      } else {
        toast(result.message || t('loginFailed'), 'error');
      }
    } finally {
      setLoginLoading(false);
    }
  }

  async function handleRegister(e: React.FormEvent) {
    e.preventDefault();
    const form = e.target as HTMLFormElement;
    const honeypot = (form.elements.namedItem('website') as HTMLInputElement)?.value;
    if (honeypot) return; // bot detected, silently reject
    if (!registerAvailable) {
      toast(t('registerClosedHint'), 'error');
      return;
    }
    if (!regUsername || !regEmail || !regPassword) return;
    setLoginLoading(true);
    try {
      const result = await register({
        username: regUsername,
        email: regEmail,
        password: regPassword,
        display_name: regDisplayName || undefined,
        turnstile_token: turnstileRef.current,
      });
      if (result.success) {
        toast(t('registerSuccess'), 'success');
        setTimeout(() => window.location.reload(), 100);
      } else {
        toast(result.message || t('registerFailed'), 'error');
      }
    } finally {
      setLoginLoading(false);
    }
  }

  /* ── MD3 输入框：无 border，surface-container-low 背景，focus 用 outline ── */
  const inputStyle: React.CSSProperties = {
    width: '100%',
    height: '48px',
    padding: '0 16px',
    border: 'none',
    borderRadius: 'var(--radius-md)',
    fontSize: '14px',
    color: 'var(--md-on-surface)',
    background: 'var(--md-surface-container-low)',
    outline: 'none',
    transition: 'background 0.2s ease, outline 0.15s ease',
    boxSizing: 'border-box' as const,
  };

  const inputSmallStyle: React.CSSProperties = {
    ...inputStyle,
    height: '44px',
    fontSize: '13px',
  };

  const focusIn = (e: React.FocusEvent<HTMLInputElement>) => {
    e.currentTarget.style.outline = '2px solid var(--md-primary)';
    e.currentTarget.style.outlineOffset = '2px';
  };
  const focusOut = (e: React.FocusEvent<HTMLInputElement>) => {
    e.currentTarget.style.outline = 'none';
  };

  /* ── MD3 实心按钮：pill 形，hover scale(0.97)，no border/shadow ── */
  const primaryBtnStyle = (loading: boolean, small = false): React.CSSProperties => ({
    width: '100%',
    height: small ? '44px' : '48px',
    background: loading ? 'var(--md-primary)' : 'var(--md-primary)',
    color: 'var(--md-on-primary)',
    border: 'none',
    borderRadius: 'var(--radius-full)',
    fontSize: small ? '14px' : '15px',
    fontWeight: 700,
    cursor: loading ? 'not-allowed' : 'pointer',
    opacity: loading ? 0.7 : 1,
    transition: 'transform 0.2s cubic-bezier(0.4, 0, 0.2, 1), background 0.15s ease',
    letterSpacing: '0.02em',
  });

  const btnHover = (e: React.MouseEvent<HTMLButtonElement>, loading: boolean) => {
    if (loading) return;
    e.currentTarget.style.background = 'var(--md-primary-dim)';
    e.currentTarget.style.transform = 'scale(0.97)';
  };
  const btnLeave = (e: React.MouseEvent<HTMLButtonElement>, loading: boolean) => {
    if (loading) return;
    e.currentTarget.style.background = 'var(--md-primary)';
    e.currentTarget.style.transform = 'scale(1)';
  };
  const btnDown = (e: React.MouseEvent<HTMLButtonElement>, loading: boolean) => {
    if (loading) return;
    e.currentTarget.style.transform = 'scale(0.95)';
  };
  const btnUp = (e: React.MouseEvent<HTMLButtonElement>, loading: boolean) => {
    if (loading) return;
    e.currentTarget.style.transform = 'scale(0.97)';
  };

  return (
    <div
      style={{
        minHeight: '100vh',
        background: 'var(--md-background)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        fontFamily: "'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', 'Noto Sans SC', 'Microsoft YaHei', 'PingFang SC', sans-serif",
        padding: '24px',
      }}
    >
      {/* 背景装饰 — 极淡渐变圆，营造氛围 */}
      <div style={{
        position: 'fixed', top: 0, left: 0, right: 0, bottom: 0,
        pointerEvents: 'none', overflow: 'hidden',
      }}>
        <div style={{
          position: 'absolute', top: '-120px', right: '-80px',
          width: '480px', height: '480px',
          borderRadius: '50%',
          background: 'radial-gradient(circle, rgba(249,115,22,0.06) 0%, transparent 70%)',
        }} />
        <div style={{
          position: 'absolute', bottom: '-100px', left: '-60px',
          width: '360px', height: '360px',
          borderRadius: '50%',
          background: 'radial-gradient(circle, rgba(67,96,138,0.04) 0%, transparent 70%)',
        }} />
      </div>

      {/* 语言切换 — MD3 pill 形，primary-container 活跃态 */}
      <div style={{
        position: 'fixed', top: '20px', right: '20px',
        display: 'flex', gap: '4px',
        background: 'var(--md-surface-container)', padding: '4px',
        borderRadius: 'var(--radius-full)',
        zIndex: 100,
      }}>
        {(['zh', 'en'] as const).map((l) => (
          <button
            key={l}
            onClick={() => setLang(l)}
            style={{
              padding: '6px 14px',
              borderRadius: 'var(--radius-full)',
              border: 'none',
              background: lang === l ? 'var(--md-primary-container)' : 'transparent',
              color: lang === l ? 'var(--md-on-primary-container)' : 'var(--md-on-surface-variant)',
              fontSize: '13px',
              fontWeight: lang === l ? 600 : 400,
              cursor: 'pointer',
              transition: 'background 0.15s ease, color 0.15s ease',
            }}
          >
            {l === 'zh' ? t('langZh') : t('langEn')}
          </button>
        ))}
      </div>

      {/* 登录卡片 — MD3: no border, surface-container-lowest, elevation-2 */}
      <div
        style={{
          background: 'var(--md-surface-container-lowest)',
          borderRadius: 'var(--radius-xl)',
          boxShadow: 'var(--elevation-2)',
          width: isMobile ? 'calc(100vw - 32px)' : '420px',
          maxWidth: '100%',
          position: 'relative',
          zIndex: 1,
          overflow: 'hidden',
        }}
      >
        {/* 渐变品牌区 — 使用 MD3 primary 变量 */}
        <div style={{
          background: 'linear-gradient(135deg, var(--md-primary) 0%, var(--md-primary) 60%, var(--md-primary-container) 100%)',
          padding: '40px 32px 36px',
          textAlign: 'center',
          position: 'relative',
          overflow: 'hidden',
        }}>
          {/* 装饰圆 */}
          <div style={{ position: 'absolute', top: '-40px', right: '-30px', width: '140px', height: '140px', borderRadius: '50%', background: 'rgba(255,255,255,0.06)' }} />
          <div style={{ position: 'absolute', bottom: '-25px', left: '20px', width: '80px', height: '80px', borderRadius: '50%', background: 'rgba(255,255,255,0.04)' }} />

          {/* Logo — 磨砂玻璃效果 */}
          <div style={{
            width: '56px', height: '56px', borderRadius: 'var(--radius-lg)',
            background: 'rgba(255,255,255,0.15)',
            backdropFilter: 'blur(8px)',
            WebkitBackdropFilter: 'blur(8px)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            margin: '0 auto 12px',
          }}>
            <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
              <path d="M12 2L2 7l10 5 10-5-10-5z" fill="white" opacity="0.95"/>
              <path d="M2 17l10 5 10-5M2 12l10 5 10-5" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" opacity="0.6"/>
            </svg>
          </div>
          {/* 标题 — Manrope 字体 */}
          <h1 style={{ color: '#ffffff', fontSize: '22px', fontWeight: 800, margin: 0, letterSpacing: '-0.5px', lineHeight: 1.15, fontFamily: "'Manrope', sans-serif" }}>{t('title')}</h1>
          <p style={{ color: 'rgba(255,255,255,0.7)', fontSize: '13px', marginTop: '6px', fontWeight: 500 }}>{t('subtitle')}</p>
        </div>

        {/* Tab 切换 — MD3: surface-container 背景, surface-container-lowest 活跃 pill, no border/shadow */}
        {showTabs ? (
          <div style={{
            display: 'flex', margin: '24px 28px 0',
            background: 'var(--md-surface-container)', borderRadius: 'var(--radius-full)', padding: '4px', position: 'relative',
          }}>
            {/* 滑块指示器 — 无阴影，tonal 区分 */}
            <div style={{
              position: 'absolute', top: '4px', left: '4px',
              width: `calc(${100 / tabs.length}% - 4px)`, height: 'calc(100% - 8px)',
              borderRadius: 'var(--radius-full)',
              background: 'var(--md-surface-container-lowest)',
              transform: `translateX(${activeTabIndex * 100}%)`,
              transition: 'transform 0.25s cubic-bezier(0.4, 0, 0.2, 1)',
            }} />
            {tabs.map((tabKey) => (
              <button
                key={tabKey}
                onClick={() => setTab(tabKey)}
                style={{
                  flex: 1, padding: '10px', textAlign: 'center',
                  fontSize: '14px', fontWeight: 600,
                  border: 'none', background: 'transparent', cursor: 'pointer',
                  color: tab === tabKey ? 'var(--md-primary)' : 'var(--md-outline)',
                  transition: 'color 0.2s', position: 'relative', zIndex: 1,
                }}
              >
                {t(tabKey)}
              </button>
            ))}
          </div>
        ) : null}

        {/* 表单区域 */}
        <div style={{ padding: showTabs ? '24px 28px 0' : '28px 28px 0' }}>
          {setupLoading ? (
            <div
              style={{
                minHeight: '132px',
                display: 'grid',
                placeItems: 'center',
                color: 'var(--md-outline)',
                fontSize: '14px',
                fontWeight: 600,
              }}
            >
              {t('checkingSetup')}
            </div>
          ) : null}

          {!setupLoading && tab === 'login' ? (
            <form onSubmit={handleLogin} autoComplete="off" style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              <input type="text" value={loginValue} onChange={e => setLoginValue(e.target.value)}
                placeholder={t('usernameOrEmail')} required style={inputStyle} onFocus={focusIn} onBlur={focusOut} />
              <input type="password" value={loginPassword} onChange={e => setLoginPassword(e.target.value)}
                placeholder={t('password')} required style={inputStyle} onFocus={focusIn} onBlur={focusOut} />
              <label style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: -4 }}>
                <input type="checkbox" checked={rememberMe} onChange={(e) => setRememberMe(e.target.checked)} />
                <span style={{ fontSize: 13, color: 'var(--md-on-surface-variant)' }}>{t('rememberMe')}</span>
              </label>
              <button
                type="submit"
                disabled={loginLoading || !turnstileReady}
                style={primaryBtnStyle(loginLoading || !turnstileReady)}
                onMouseEnter={e => btnHover(e, loginLoading || !turnstileReady)}
                onMouseLeave={e => btnLeave(e, loginLoading || !turnstileReady)}
                onMouseDown={e => btnDown(e, loginLoading || !turnstileReady)}
                onMouseUp={e => btnUp(e, loginLoading || !turnstileReady)}
              >
                {loginLoading ? t('loggingIn') : t('loginBtn')}
              </button>
            </form>
          ) : null}

          {!setupLoading && tab === 'register' ? (
            <form onSubmit={handleRegister} autoComplete="off" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ position: 'absolute', left: '-9999px', opacity: 0 }} aria-hidden="true">
                <label htmlFor="honeypot_website">Leave this empty</label>
                <input id="honeypot_website" name="website" type="text" tabIndex={-1} autoComplete="off" />
              </div>
              <input type="text" value={regUsername} onChange={e => setRegUsername(e.target.value)}
                placeholder={t('username')} required style={inputSmallStyle} onFocus={focusIn} onBlur={focusOut} />
              <input type="email" value={regEmail} onChange={e => setRegEmail(e.target.value)}
                placeholder={t('email')} required style={inputSmallStyle} onFocus={focusIn} onBlur={focusOut} />
              <input type="password" value={regPassword} onChange={e => setRegPassword(e.target.value)}
                placeholder={t('password')} required style={inputSmallStyle} onFocus={focusIn} onBlur={focusOut} />
              <input type="text" value={regDisplayName} onChange={e => setRegDisplayName(e.target.value)}
                placeholder={t('displayName')} style={inputSmallStyle} onFocus={focusIn} onBlur={focusOut} />
              <button
                type="submit"
                disabled={loginLoading || !turnstileReady}
                style={{ ...primaryBtnStyle(loginLoading || !turnstileReady, true), marginTop: '4px' }}
                onMouseEnter={e => btnHover(e, loginLoading || !turnstileReady)}
                onMouseLeave={e => btnLeave(e, loginLoading || !turnstileReady)}
                onMouseDown={e => btnDown(e, loginLoading || !turnstileReady)}
                onMouseUp={e => btnUp(e, loginLoading || !turnstileReady)}
              >
                {loginLoading ? t('creating') : t('registerBtn')}
              </button>
            </form>
          ) : null}
        </div>

        <div style={{ padding: '0 28px' }}>
          <SlotRenderer target="login.form_below" />
        </div>

        {/* Turnstile 验证码 widget */}
        <div style={{ padding: '0 28px', marginTop: '8px' }}>
          <div
            id="turnstile-widget"
            style={{
              minHeight: '65px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              background: 'var(--md-surface-container-low)',
              borderRadius: 'var(--radius-md)',
              padding: '12px',
            }}
          ></div>
          {!turnstileReady && (
            <div
              style={{
                textAlign: 'center',
                fontSize: '12px',
                color: 'var(--md-outline)',
                marginTop: '6px',
              }}
            >
              {t('loadingTurnstile')}
            </div>
          )}
        </div>

        {/* 底部链接 */}
        <div style={{ textAlign: 'center', paddingTop: '20px', paddingBottom: '28px' }}>
          <a href="/"
            style={{
              fontSize: '13px',
              color: 'var(--md-outline)',
              textDecoration: 'none',
              display: 'inline-flex',
              alignItems: 'center',
              gap: '4px',
              transition: 'color 0.15s ease',
            }}
            onMouseEnter={e => e.currentTarget.style.color = 'var(--md-primary)'}
            onMouseLeave={e => e.currentTarget.style.color = 'var(--md-outline)'}
          >
            ← {t('backToHome')}
          </a>
        </div>
      </div>
    </div>
  );
}

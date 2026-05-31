import { useAuth } from '../contexts/AuthContext';
import { useI18n } from '../i18n';
import { useSlots } from '../lib/slots';
import {
  IconFileText, IconFolderOpen, IconTag, IconMessageSquare,
  IconUpload, IconSettings, IconUser, IconLogOut, IconPalette, IconTrash2, IconPackage, IconZap,
} from './Icons';

// 导航配置类型
interface NavItemConfig {
  key: string;
  icon: React.FC<{ size?: number }>;
  labelKey: string;
}

interface NavGroupConfig {
  sectionKey: string;
  items: NavItemConfig[];
}

// hover 预加载页面 chunk
const prefetchPages: Record<string, () => void> = {
  posts: () => import('../pages/Posts'),
  categories: () => import('../pages/Categories'),
  tags: () => import('../pages/Tags'),
  comments: () => import('../pages/CommentsV2'),
  settings: () => import('../pages/Settings'),
  'fab-settings': () => import('../pages/FabSettings'),
  upload: () => import('../pages/Upload'),
  themes: () => import('../pages/Themes'),
  trash: () => import('../pages/RecycleBin'),
  'media-categories': () => import('../pages/MediaCategories'),
  plugins: () => import('../pages/PluginManager'),
};

// 导航配置（key 用于匹配路由，labelKey 用于翻译）
const navConfig: NavGroupConfig[] = [
  {
    sectionKey: 'content',
    items: [
      { key: 'posts', icon: IconFileText, labelKey: 'posts' },
      { key: 'categories', icon: IconFolderOpen, labelKey: 'categories' },
      { key: 'tags', icon: IconTag, labelKey: 'tags' },
      { key: 'comments', icon: IconMessageSquare, labelKey: 'comments' },
    ],
  },
  {
    sectionKey: 'system',
    items: [
      { key: 'upload', icon: IconUpload, labelKey: 'upload' },
      { key: 'media-categories', icon: IconFolderOpen, labelKey: 'mediaCategories' },
      { key: 'themes', icon: IconPalette, labelKey: 'themes' },
      { key: 'plugins', icon: IconPackage, labelKey: 'plugins' },
      { key: 'trash', icon: IconTrash2, labelKey: 'trash' },
      { key: 'settings', icon: IconSettings, labelKey: 'settings' },
      { key: 'fab-settings', icon: IconZap, labelKey: 'fabSettings' },
    ],
  },
];

interface SidebarProps {
  activePage: string;
  onNavigate: (page: string) => void;
}

export function Sidebar({ activePage, onNavigate }: SidebarProps) {
  const { user, logout } = useAuth();
  const { lang, setLang, t } = useI18n();
  const { slots } = useSlots();
  const menuSlots = slots.filter(s => s.target === 'sidebar.menu_item');

  return (
    <aside
      style={{
        width: '288px',
        background: 'var(--sidebar-bg)',
        color: 'var(--md-on-surface)',
        display: 'flex',
        flexDirection: 'column',
        flexShrink: 0,
        height: '100vh',
        overflowY: 'auto',
        overflowX: 'hidden',
        position: 'relative',
        zIndex: 10,
        borderRight: 'none',
      }}
    >
      {/* ── 顶部品牌区 ── */}
      <div
        onClick={() => onNavigate('posts')}
        style={{
          padding: '24px 20px',
          display: 'flex',
          alignItems: 'center',
          gap: '12px',
          cursor: 'pointer',
          transition: 'background 0.15s ease',
        }}
        onMouseEnter={e => (e.currentTarget.style.background = 'var(--sidebar-hover)')}
        onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
      >
        {/* Logo — 使用实际 logo-icon.svg，自带完整色彩 */}
        <div style={{
          width: '40px', height: '40px',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          flexShrink: 0,
        }}>
          <img src="/static/themes/default/logo-icon.svg" alt="InkForge" style={{ width: '36px', height: '36px' }} />
        </div>
        <div>
          <div style={{
            fontSize: '16px',
            fontWeight: 900,
            color: 'var(--md-on-surface)',
            lineHeight: 1.2,
            fontFamily: "'Manrope', sans-serif",
            letterSpacing: '-0.3px',
          }}>
            InkForge
          </div>
          <div style={{
            fontSize: '10px',
            color: 'var(--md-on-surface-variant)',
            marginTop: '2px',
            textTransform: 'uppercase',
            letterSpacing: '0.12em',
            fontWeight: 500,
          }}>
            {t('adminPanel')}
          </div>
        </div>
      </div>

      {/* ── 导航 — Pill 风格，无左侧指示条 ── */}
      <nav style={{ flex: 1, padding: '4px 12px', overflowY: 'auto' }}>
        {navConfig.map(group => (
          <div key={group.sectionKey} style={{ marginBottom: '4px' }}>
            {/* 分组标题 */}
            <div style={{
              padding: '16px 12px 8px',
              fontSize: '11px',
              fontWeight: 700,
              color: 'var(--md-on-surface-variant)',
              textTransform: 'uppercase',
              letterSpacing: '0.1em',
            }}>
              {t(group.sectionKey)}
            </div>
            {/* 菜单项 — Pill 形状 */}
            {group.items.map(item => {
              const isActive = activePage === item.key;
              const IconComponent = item.icon;
              return (
                <button
                  key={item.key}
                  onClick={() => onNavigate(item.key)}
                  style={{
                    width: '100%',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '12px',
                    padding: '10px 16px',
                    borderRadius: 'var(--radius-full)',
                    border: 'none',
                    background: isActive ? 'var(--md-primary-container)' : 'transparent',
                    color: isActive ? 'var(--md-on-primary-container)' : 'var(--sidebar-text)',
                    fontSize: '14px',
                    fontWeight: isActive ? 600 : 400,
                    cursor: 'pointer',
                    transition: 'all 0.15s ease',
                    textAlign: 'left',
                    marginBottom: '2px',
                    position: 'relative',
                  }}
                  onMouseEnter={e => {
                    if (!isActive) {
                      prefetchPages[item.key]?.();
                      e.currentTarget.style.background = 'var(--sidebar-hover)';
                      e.currentTarget.style.color = 'var(--sidebar-text-hover)';
                    }
                  }}
                  onMouseLeave={e => {
                    if (!isActive) {
                      e.currentTarget.style.background = 'transparent';
                      e.currentTarget.style.color = 'var(--sidebar-text)';
                    }
                  }}
                >
                  <span style={{
                    width: '20px',
                    textAlign: 'center',
                    flexShrink: 0,
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                  }}>
                    <IconComponent />
                  </span>
                  <span>{t(item.labelKey)}</span>
                </button>
              );
            })}
          </div>
        ))}
      </nav>

      {/* ── 底部用户区 — 无 border-top，靠间距分隔 ── */}
      <div style={{ padding: '16px 12px' }}>
        {/* 语言切换 — pill 按钮 */}
        <div style={{
          display: 'flex',
          gap: '4px',
          padding: '0 8px 16px',
          justifyContent: 'center',
        }}>
          {(['zh', 'en'] as const).map((l) => (
            <button
              key={l}
              onClick={() => setLang(l)}
              style={{
                padding: '6px 16px',
                borderRadius: 'var(--radius-full)',
                border: 'none',
                background: lang === l ? 'var(--md-primary-container)' : 'transparent',
                color: lang === l ? 'var(--md-on-primary-container)' : 'var(--md-on-surface-variant)',
                fontSize: '12px',
                fontWeight: lang === l ? 600 : 400,
                cursor: 'pointer',
                transition: 'all 0.15s ease',
              }}
            >
              {l === 'zh' ? t('langZh') : t('langEn')}
            </button>
          ))}
        </div>
        {/* 用户信息 */}
        <div style={{
          display: 'flex',
          alignItems: 'center',
          gap: '12px',
          padding: '10px 12px',
          marginBottom: '4px',
        }}>
          <div style={{
            width: '36px', height: '36px', borderRadius: 'var(--radius-full)',
            background: 'var(--md-primary-container)',
            display: 'flex', alignItems: 'center', justifyContent: 'center',
            flexShrink: 0,
            color: 'var(--md-on-primary-container)',
          }}>
            <IconUser size={16} />
          </div>
          <div style={{ minWidth: 0 }}>
            <div style={{
              fontSize: '13px',
              fontWeight: 600,
              color: 'var(--md-on-surface)',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}>
              {user?.display_name || t('admin')}
            </div>
            <div style={{ fontSize: '11px', color: 'var(--md-on-surface-variant)' }}>
              {user?.role === 'admin' ? t('admin') : t('member')}
            </div>
          </div>
        </div>
        <a
          href={window.location.origin + '/'}
          target="_blank"
          rel="noopener noreferrer"
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '8px 12px',
            borderRadius: 'var(--radius-full)',
            border: 'none',
            background: 'transparent',
            color: 'var(--md-on-surface-variant)',
            fontSize: '13px',
            fontWeight: 500,
            cursor: 'pointer',
            textDecoration: 'none',
            transition: 'all 0.15s ease',
            textAlign: 'left',
            marginBottom: '4px',
          }}
          onMouseEnter={e => {
            e.currentTarget.style.background = 'var(--sidebar-hover)';
            e.currentTarget.style.color = 'var(--md-primary)';
          }}
          onMouseLeave={e => {
            e.currentTarget.style.background = 'transparent';
            e.currentTarget.style.color = 'var(--md-on-surface-variant)';
          }}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
            <polyline points="15 3 21 3 21 9"/>
            <line x1="10" y1="14" x2="21" y2="3"/>
          </svg>
          <span>{t('visitSite')}</span>
        </a>
        {/* 插件菜单项 */}
        {menuSlots.map(s => (
          <a
            key={s.plugin_name}
            href={s.entry}
            target="_blank"
            rel="noopener noreferrer"
            style={{
              width: '100%',
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              padding: '8px 12px',
              borderRadius: 'var(--radius-full)',
              textDecoration: 'none',
              color: 'var(--md-on-surface-variant)',
              fontSize: '13px',
              fontWeight: 500,
              cursor: 'pointer',
              transition: 'all 0.15s ease',
              marginBottom: '4px',
            }}
            onMouseEnter={e => {
              e.currentTarget.style.background = 'var(--sidebar-hover)';
              e.currentTarget.style.color = 'var(--md-primary)';
            }}
            onMouseLeave={e => {
              e.currentTarget.style.background = 'transparent';
              e.currentTarget.style.color = 'var(--md-on-surface-variant)';
            }}
          >
            {s.label}
          </a>
        ))}
        {/* 退出按钮 — 文字链接风格 */}
        <button
          onClick={() => void logout()}
          style={{
            width: '100%',
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '8px 12px',
            borderRadius: 'var(--radius-full)',
            border: 'none',
            background: 'transparent',
            color: 'var(--md-on-surface-variant)',
            fontSize: '13px',
            fontWeight: 500,
            cursor: 'pointer',
            transition: 'all 0.15s ease',
            textAlign: 'left',
          }}
          onMouseEnter={e => {
            e.currentTarget.style.background = 'var(--sidebar-hover)';
            e.currentTarget.style.color = 'var(--md-error)';
          }}
          onMouseLeave={e => {
            e.currentTarget.style.background = 'transparent';
            e.currentTarget.style.color = 'var(--md-on-surface-variant)';
          }}
        >
          <IconLogOut />
          <span>{t('logout')}</span>
        </button>
      </div>
    </aside>
  );
}

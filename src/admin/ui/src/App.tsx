import { lazy, Suspense, useState, useEffect } from 'react';
import { BrowserRouter, Navigate, Outlet, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { useAuth } from './contexts/AuthContext';
import { useResponsive } from './hooks/useResponsive';
import { Sidebar } from './components/Sidebar';
import { PostsSkeleton } from './components/Skeleton';
import { SlotsContext, type SlotInfo } from './lib/slots';
import { PreviewProvider } from './preview';
import { FabContainer } from './fab';
import { IconMenu } from './components/Icons';

// 关键首屏页面保持 eager，Login 是非登录态的首屏、Setup 是首次安装
import Login from './pages/Login';
import Setup from './pages/Setup';

// ── 13 个管理页面全部 lazy 加载，按路由分 chunk ──
const Posts = lazy(() => import('./pages/Posts'));
const PostEditor = lazy(() => import('./pages/PostEditor'));
const Categories = lazy(() => import('./pages/Categories'));
const Tags = lazy(() => import('./pages/Tags'));
const Comments = lazy(() => import('./pages/CommentsV2'));
const Settings = lazy(() => import('./pages/Settings'));
const Upload = lazy(() => import('./pages/Upload'));
const MediaCategories = lazy(() => import('./pages/MediaCategories'));
const Themes = lazy(() => import('./pages/Themes'));
const ThemeDetail = lazy(() => import('./pages/ThemeDetail'));
const RecycleBin = lazy(() => import('./pages/RecycleBin'));
const PluginSettings = lazy(() => import('./pages/PluginSettings'));
const PluginManager = lazy(() => import('./pages/PluginManager'));
const FabSettings = lazy(() => import('./pages/FabSettings'));
const Webhooks = lazy(() => import('./pages/Webhooks'));
const ApiKeys = lazy(() => import('./pages/ApiKeys'));

const pageToRoute: Record<string, string> = {
  posts: '/posts',
  categories: '/categories',
  tags: '/tags',
  comments: '/comments',
  settings: '/settings',
  'fab-settings': '/fab-settings',
  upload: '/upload',
  'media-categories': '/media-categories',
  themes: '/themes',
  plugins: '/plugins',
  webhooks: '/webhooks',
  'api-keys': '/api-keys',
  trash: '/trash',
};

function getActivePage(pathname: string): string {
  if (pathname.startsWith('/posts')) return 'posts';
  if (pathname.startsWith('/themes')) return 'themes';
  if (pathname.startsWith('/plugins')) return 'plugins';
  if (pathname.startsWith('/webhooks')) return 'webhooks';
  if (pathname.startsWith('/api-keys')) return 'api-keys';
  if (pathname.startsWith('/categories')) return 'categories';
  if (pathname.startsWith('/tags')) return 'tags';
  if (pathname.startsWith('/comments')) return 'comments';
  if (pathname.startsWith('/settings')) return 'settings';
  if (pathname.startsWith('/fab-settings')) return 'fab-settings';
  if (pathname.startsWith('/upload')) return 'upload';
  if (pathname.startsWith('/media-categories')) return 'media-categories';
  if (pathname.startsWith('/trash')) return 'trash';
  return 'posts';
}

function AdminLayout() {
  const location = useLocation();
  const navigate = useNavigate();
  const activePage = getActivePage(location.pathname);
  const { isMobile, isTablet, isDesktop } = useResponsive();
  const [mobileOpen, setMobileOpen] = useState(false);

  // 切换路由时关闭移动端侧边栏
  useEffect(() => {
    setMobileOpen(false);
  }, [location.pathname]);

  return (
    <div style={{ display: 'flex', height: '100vh', overflow: 'hidden' }}>
      <Sidebar
        activePage={activePage}
        onNavigate={(page) => navigate(pageToRoute[page] || '/posts')}
        mobileOpen={mobileOpen}
        setMobileOpen={setMobileOpen}
        isMobile={isMobile}
        isTablet={isTablet}
        isDesktop={isDesktop}
      />

      {/* 移动端 hamburger 按钮 */}
      {isMobile && (
        <button
          onClick={() => setMobileOpen(true)}
          style={{
            position: 'fixed',
            top: '12px',
            left: '12px',
            zIndex: 999,
            width: '40px',
            height: '40px',
            borderRadius: '50%',
            background: 'var(--md-surface-container)',
            border: 'none',
            cursor: 'pointer',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: 'var(--md-on-surface)',
            boxShadow: '0 1px 4px rgba(0,0,0,0.12)',
          }}
        >
          <IconMenu size={20} />
        </button>
      )}

      {/* 移动端 overlay 蒙层 */}
      {isMobile && mobileOpen && (
        <div
          onClick={() => setMobileOpen(false)}
          style={{
            position: 'fixed',
            inset: 0,
            zIndex: 999,
            background: 'rgba(0,0,0,0.5)',
          }}
        />
      )}

      {/* 主内容区 */}
      <main style={{
        flex: 1,
        padding: isMobile ? '16px' : '32px',
        background: 'var(--bg-base)',
        overflow: 'auto',
        minHeight: 0,
        marginLeft: isMobile ? 0 : undefined,
      }}>
        <Suspense fallback={<PostsSkeleton />}>
          <Outlet />
        </Suspense>
      </main>

      <FabContainer />
    </div>
  );
}

function AdminGate() {
  const { user, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="min-h-screen p-8" style={{ background: 'var(--bg-base)' }}>
        <PostsSkeleton />
      </div>
    );
  }

  if (!user) {
    return <Login />;
  }

  return <AdminLayout />;
}

function SlotsProvider({ children }: { children: React.ReactNode }) {
  const [slots, setSlots] = useState<SlotInfo[]>([]);
  useEffect(() => {
    fetch('/api/v1/admin/plugins/slots', { credentials: 'include' })
      .then(r => r.json())
      .then(d => setSlots(d.data?.slots || []))
      .catch(() => {});
  }, []);
  return <SlotsContext.Provider value={{ slots }}>{children}</SlotsContext.Provider>;
}

export default function App() {
  return (
    <BrowserRouter basename="/admin">
      <SlotsProvider>
      <PreviewProvider>
      <Routes>
        <Route path="/setup" element={<Setup />} />
        <Route path="/" element={<AdminGate />}>
          <Route index element={<Navigate to="posts" replace />} />
          <Route path="posts" element={<Posts />} />
          <Route path="posts/new" element={<PostEditor />} />
          <Route path="posts/:id/edit" element={<PostEditor />} />
          <Route path="categories" element={<Categories />} />
          <Route path="tags" element={<Tags />} />
          <Route path="comments" element={<Comments />} />
          <Route path="settings" element={<Settings />} />
          <Route path="fab-settings" element={<FabSettings />} />
          <Route path="upload" element={<Upload />} />
          <Route path="media-categories" element={<MediaCategories />} />
          <Route path="themes" element={<Themes />} />
          <Route path="themes/:slug" element={<ThemeDetail />} />
          <Route path="trash" element={<RecycleBin />} />
          <Route path="plugins/:name/settings" element={<PluginSettings />} />
          <Route path="plugins" element={<PluginManager />} />
          <Route path="webhooks" element={<Webhooks />} />
          <Route path="api-keys" element={<ApiKeys />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
      </PreviewProvider>
      </SlotsProvider>
    </BrowserRouter>
  );
}

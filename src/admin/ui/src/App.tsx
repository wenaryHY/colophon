import { lazy, Suspense, useState, useEffect } from 'react';
import { BrowserRouter, Navigate, Outlet, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { useAuth } from './contexts/AuthContext';
import { Sidebar } from './components/Sidebar';
import { PostsSkeleton } from './components/Skeleton';
import { SlotsContext, type SlotInfo } from './lib/slots';

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
  trash: '/trash',
};

function getActivePage(pathname: string): string {
  if (pathname.startsWith('/posts')) return 'posts';
  if (pathname.startsWith('/themes')) return 'themes';
  if (pathname.startsWith('/plugins')) return 'plugins';
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

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar
        activePage={activePage}
        onNavigate={(page) => navigate(pageToRoute[page] || '/posts')}
      />
      <main className="flex-1 overflow-y-auto" style={{ padding: '20px 24px', background: 'var(--bg-base)' }}>
        <Suspense fallback={<PostsSkeleton />}>
          <Outlet />
        </Suspense>
      </main>
    </div>
  );
}

function AdminGate() {
  const { token, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="min-h-screen p-8" style={{ background: 'var(--bg-base)' }}>
        <PostsSkeleton />
      </div>
    );
  }

  if (!token) {
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
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
      </SlotsProvider>
    </BrowserRouter>
  );
}

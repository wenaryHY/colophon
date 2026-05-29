import { BrowserRouter, Navigate, Outlet, Route, Routes, useLocation, useNavigate } from 'react-router-dom';
import { useAuth } from './contexts/AuthContext';
import { Sidebar } from './components/Sidebar';
import { PostsSkeleton } from './components/Skeleton';
import Login from './pages/Login';
import Posts from './pages/Posts';
import PostEditor from './pages/PostEditor';
import Categories from './pages/Categories';
import Tags from './pages/Tags';
import Comments from './pages/CommentsV2';
import Settings from './pages/Settings';
import Upload from './pages/Upload';
import MediaCategories from './pages/MediaCategories';
import Themes from './pages/Themes';
import ThemeDetail from './pages/ThemeDetail';
import RecycleBin from './pages/RecycleBin';
import Setup from './pages/Setup';
import PluginSettings from './pages/PluginSettings';

const pageToRoute: Record<string, string> = {
  posts: '/posts',
  categories: '/categories',
  tags: '/tags',
  comments: '/comments',
  settings: '/settings',
  upload: '/upload',
  'media-categories': '/media-categories',
  themes: '/themes',
  trash: '/trash',
};

function getActivePage(pathname: string): string {
  if (pathname.startsWith('/posts')) return 'posts';
  if (pathname.startsWith('/themes')) return 'themes';
  if (pathname.startsWith('/categories')) return 'categories';
  if (pathname.startsWith('/tags')) return 'tags';
  if (pathname.startsWith('/comments')) return 'comments';
  if (pathname.startsWith('/settings')) return 'settings';
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
      <main className="flex-1 overflow-y-auto" style={{ padding: '24px 32px', background: 'var(--bg-base)' }}>
        <Outlet />
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

export default function App() {
  return (
    <BrowserRouter basename="/admin">
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
          <Route path="upload" element={<Upload />} />
          <Route path="media-categories" element={<MediaCategories />} />
          <Route path="themes" element={<Themes />} />
          <Route path="themes/:slug" element={<ThemeDetail />} />
          <Route path="trash" element={<RecycleBin />} />
          <Route path="plugins/:name/settings" element={<PluginSettings />} />
        </Route>
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </BrowserRouter>
  );
}

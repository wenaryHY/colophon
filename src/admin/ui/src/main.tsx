import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import './index.css'
import App from './App'
import { AuthProvider } from './contexts/AuthContext'
import { ToastProvider } from './contexts/ToastContext'
import { I18nProvider } from './i18n'
import { setQueryClient } from './lib/api'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,          // 30s 内不重新获取
      retry: 1,                    // 失败重试 1 次
      refetchOnWindowFocus: false, // 窗口聚焦不重新获取
    },
  },
});

setQueryClient(queryClient);

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <I18nProvider>
          <AuthProvider>
            <App />
          </AuthProvider>
        </I18nProvider>
      </ToastProvider>
    </QueryClientProvider>
  </StrictMode>,
)

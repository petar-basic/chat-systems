import { lazy, Suspense, useEffect } from 'react';
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClientProvider } from '@tanstack/react-query';
import { ReactQueryDevtools } from '@tanstack/react-query-devtools';
import { queryClient } from './lib/queryClient';
import { useWebSocketQuerySync } from './lib/wsQuerySync';
import { useInstanceStore } from './stores/instances';
import { ErrorBoundary } from './components/ErrorBoundary';
import { Toaster } from '@/shared/components/Toast';
import { NotificationStream } from '@/features/notifications';
import { HuddleController } from '@/features/huddle';
import { logger } from '@/lib/logger';
import { isSessionExpired } from '@/lib/errors';

const AddInstancePage = lazy(() => import('./pages/AddInstancePage'));
const CompleteRegistrationPage = lazy(() => import('./pages/CompleteRegistrationPage'));
const WorkspacePage = lazy(() => import('./pages/WorkspacePage'));
const ResetPasswordPage = lazy(() => import('./pages/ResetPasswordPage'));
const InstanceAdminPage = lazy(() => import('./pages/InstanceAdminPage'));

function SplashScreen() {
  return (
    <div className="h-screen flex flex-col items-center justify-center bg-slate-900">
      <div className="w-12 h-12 rounded-2xl bg-purple-600 flex items-center justify-center text-2xl font-bold text-white mb-4">
        C
      </div>
      <div className="w-6 h-6 border-2 border-purple-500/30 border-t-purple-500 rounded-full animate-spin" />
    </div>
  );
}

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const hydrated = useInstanceStore((s) => s.hydrated);
  const instances = useInstanceStore((s) => s.instances);

  if (!hydrated) return <SplashScreen />;
  if (instances.length === 0) return <Navigate to="/add-instance" replace />;
  return <>{children}</>;
}

function AppContent() {
  const { hydrated, restoreInstances, instances } = useInstanceStore();

  useEffect(() => {
    void restoreInstances();
  }, [restoreInstances]);

  useEffect(() => {
    const onError = (event: ErrorEvent) => {
      logger.error('App', 'window.error', event.error ?? event.message);
    };
    const onUnhandledRejection = (event: PromiseRejectionEvent) => {
      if (isSessionExpired(event.reason)) return;
      logger.error('App', 'window.unhandledrejection', event.reason);
    };
    window.addEventListener('error', onError);
    window.addEventListener('unhandledrejection', onUnhandledRejection);
    return () => {
      window.removeEventListener('error', onError);
      window.removeEventListener('unhandledrejection', onUnhandledRejection);
    };
  }, []);

  useWebSocketQuerySync();

  if (!hydrated) {
    return <SplashScreen />;
  }

  return (
    <BrowserRouter>
      <NotificationStream />
      <HuddleController />
      <Suspense fallback={<SplashScreen />}>
        <Routes>
          <Route path="/add-instance" element={<AddInstancePage />} />
          <Route path="/invite/:token" element={<CompleteRegistrationPage />} />
          <Route path="/complete-registration" element={<CompleteRegistrationPage />} />
          <Route path="/forgot-password" element={<ResetPasswordPage />} />
          <Route path="/reset-password" element={<ResetPasswordPage />} />
          <Route
            path="/app/admin"
            element={
              <ProtectedRoute>
                <InstanceAdminPage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/app/:workspaceId/dm/:dmUserId"
            element={
              <ProtectedRoute>
                <WorkspacePage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/app/:workspaceId/:channelId/:messageId"
            element={
              <ProtectedRoute>
                <WorkspacePage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/app/:workspaceId/:channelId"
            element={
              <ProtectedRoute>
                <WorkspacePage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/app/:workspaceId?"
            element={
              <ProtectedRoute>
                <WorkspacePage />
              </ProtectedRoute>
            }
          />
          <Route
            path="/"
            element={<Navigate to={instances.length === 0 ? '/add-instance' : '/app'} replace />}
          />
          <Route
            path="/login"
            element={<Navigate to={instances.length === 0 ? '/add-instance' : '/app'} replace />}
          />
          <Route path="*" element={<Navigate to="/app" replace />} />
        </Routes>
      </Suspense>
    </BrowserRouter>
  );
}

export default function App() {
  return (
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <AppContent />
        <Toaster />
        <ReactQueryDevtools initialIsOpen={false} />
      </QueryClientProvider>
    </ErrorBoundary>
  );
}

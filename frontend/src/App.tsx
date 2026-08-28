import { Routes, Route, Navigate, useLocation } from "react-router-dom";
import { useTheme } from "@/hooks/useTheme";
import { useAuthStore } from "@/stores/authStore";
import { useSessionStore } from "@/stores/sessionStore";
import { apiClient, isAdminRequiredError } from "@/api/client";
import { AppLayout } from "@/components/layout/AppLayout";
import { FullWidthLayout } from "@/components/layout/FullWidthLayout";
import { AdminAuthOverlay } from "@/components/auth/AdminAuthOverlay";
import { BackendOfflineOverlay } from "@/components/auth/BackendOfflineOverlay";
import { RequestsPage } from "@/pages/RequestsPage";
import { ResponseEditorPage } from "@/pages/ResponseEditorPage";
import { DnsSettingsPage } from "@/pages/DnsSettingsPage";
import { NotificationsPage } from "@/pages/NotificationsPage";
import { LoginPage } from "@/pages/LoginPage";
import { ProfilePage } from "@/pages/ProfilePage";
import { AdminPage } from "@/pages/AdminPage";

// Redirect that preserves query params (for share links)
function RedirectWithParams() {
  const location = useLocation();
  return <Navigate to={`/requests${location.search}`} replace />;
}

function App() {
  // Initialize theme (adds dark class to html element)
  useTheme();

  // Auth state
  const showAuthOverlay = useAuthStore((s) => s.showAuthOverlay);
  const backendOffline = useAuthStore((s) => s.backendOffline);
  const setShowAuthOverlay = useAuthStore((s) => s.setShowAuthOverlay);
  const setAuthError = useAuthStore((s) => s.setAuthError);
  const addSession = useSessionStore((s) => s.addSession);

  // Handle auth form submission
  const handleAuthSubmit = async (password: string) => {
    try {
      const response = await apiClient.createSession(password);
      addSession({
        subdomain: response.subdomain,
        token: response.token,
        createdAt: new Date().toISOString(),
      });
      setShowAuthOverlay(false);
      setAuthError(null);
    } catch (error) {
      if (isAdminRequiredError(error)) {
        setAuthError("Invalid admin password");
      } else {
        setAuthError("Failed to authenticate. Please try again.");
      }
    }
  };

  return (
    <main className="h-full text-foreground bg-background">
      {backendOffline && <BackendOfflineOverlay />}
      {showAuthOverlay && <AdminAuthOverlay onSubmit={handleAuthSubmit} />}
      <Routes>
        {/* Login page - no auth required */}
        <Route path="/login" element={<LoginPage />} />

        {/* Auth routes (OAuth callbacks) - handled by backend */}
        {/* /auth/* routes are handled server-side */}

        {/* Main app - with sidebar */}
        <Route path="/" element={<AppLayout />}>
          <Route index element={<RedirectWithParams />} />
          <Route path="requests" element={<RequestsPage />} />
          <Route path="response" element={<ResponseEditorPage />} />
          <Route path="dns" element={<DnsSettingsPage />} />
          <Route path="notifications" element={<NotificationsPage />} />
          {/* Catch-all: redirect unknown routes to /requests */}
          <Route path="*" element={<Navigate to="/requests" replace />} />
        </Route>

        {/* Full-width pages (no sidebar) */}
        <Route path="/profile" element={<FullWidthLayout />}>
          <Route index element={<ProfilePage />} />
        </Route>
        <Route path="/admin" element={<FullWidthLayout />}>
          <Route index element={<AdminPage />} />
        </Route>
      </Routes>
    </main>
  );
}

export default App;

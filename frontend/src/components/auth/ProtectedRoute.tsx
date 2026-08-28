import { useEffect, useState } from "react";
import { Navigate, useLocation } from "react-router-dom";
import { Spinner } from "@heroui/react";

interface UserProfile {
  username: string;
  avatar_url: string;
  name: string;
  created_at: string;
  is_admin: boolean;
}

interface ProtectedRouteProps {
  children: React.ReactNode;
  adminOnly?: boolean;
}

export function ProtectedRoute({ children, adminOnly = false }: ProtectedRouteProps) {
  const location = useLocation();
  const [authState, setAuthState] = useState<{
    checked: boolean;
    user: UserProfile | null;
  }>({ checked: false, user: null });

  useEffect(() => {
    const checkAuth = async () => {
      try {
        const res = await fetch("/auth/user");
        const data = await res.json();
        setAuthState({ checked: true, user: data.authenticated ? data.user : null });
      } catch {
        // Auth endpoints might not exist (no OAuth configured)
        // Check if admin token is being used instead
        setAuthState({ checked: true, user: null });
      }
    };
    checkAuth();
  }, []);

  if (!authState.checked) {
    return (
      <div className="flex h-screen items-center justify-center">
        <Spinner size="lg" />
      </div>
    );
  }

  // If no auth endpoints / no user, fall through (admin token mode)
  // The backend handles auth via ADMIN_TOKEN; frontend just shows the app
  if (authState.user === null) {
    // Could be admin-token mode or no auth required
    // Check if we have sessions in localStorage (admin token already used)
    try {
      const stored = localStorage.getItem("requestrepo-sessions");
      if (stored) {
        const parsed = JSON.parse(stored);
        if (parsed?.state?.sessions?.length > 0) {
          // Has sessions, allow access
          if (adminOnly) {
            // Would need to check admin status - allow for now, backend enforces
            return <>{children}</>;
          }
          return <>{children}</>;
        }
      }
    } catch {}

    // No sessions and no auth - redirect to login
    return <Navigate to="/login" state={{ from: location }} replace />;
  }

  // Authenticated - check admin requirement
  if (adminOnly && !authState.user.is_admin) {
    return <Navigate to="/" replace />;
  }

  return <>{children}</>;
}

import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { Button, Input } from "@heroui/react";
import { Eye, EyeOff, AlertCircle } from "lucide-react";
import { Doodles } from "@/components/ui/Doodles";
import { useTheme } from "@/hooks/useTheme";
import { apiClient } from "@/api/client";
import { useSessionStore } from "@/stores/sessionStore";
import { useAuthStore } from "@/stores/authStore";

interface ProviderInfo {
  name: string;
  display_name: string;
  icon: string;
  login_url: string;
  provider_type: string;
}

export function LoginPage() {
  const navigate = useNavigate();
  const { resolvedTheme } = useTheme();
  const addSession = useSessionStore((s) => s.addSession);
  const setShowAuthOverlay = useAuthStore((s) => s.setShowAuthOverlay);
  const setBackendOffline = useAuthStore((s) => s.setBackendOffline);

  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  useEffect(() => {
    const fetchProviders = async () => {
      try {
        const res = await fetch("/auth/providers");
        if (res.ok) {
          const data = await res.json();
          setProviders(data.providers || []);
        }
      } catch {
        // Auth endpoints might not be available
      } finally {
        setLoading(false);
      }
    };
    fetchProviders();
  }, []);

  const handlePasswordLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password.trim() || isSubmitting) return;

    setIsSubmitting(true);
    setAuthError(null);

    try {
      const response = await apiClient.createSession(password);
      addSession({
        subdomain: response.subdomain,
        token: response.token,
        createdAt: new Date().toISOString(),
      });
      setBackendOffline(false);
      setShowAuthOverlay(false);
      navigate("/");
    } catch {
      setAuthError("Invalid admin password");
    } finally {
      setIsSubmitting(false);
    }
  };

  if (loading) {
    return (
      <div
        style={{
          position: "fixed",
          top: 0,
          left: 0,
          width: "100%",
          height: "100%",
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          background: "#0c0a09",
          zIndex: 9999,
        }}
      >
        <div
          style={{
            width: "40px",
            height: "40px",
            border: "1px solid #262626",
            borderTopColor: "#F6D30F",
            borderRadius: "50%",
            animation: "spin 0.8s linear infinite",
          }}
        />
        <style>{`@keyframes spin { to { transform: rotate(360deg); } }`}</style>
      </div>
    );
  }

  const showOAuth = providers.length > 0;

  return (
    <div
      style={{
        minHeight: "100vh",
        position: "relative",
        background: "#0c0a09",
      }}
    >
      <Doodles count={120} />

      <div
        style={{
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          minHeight: "100vh",
          padding: "20px",
          position: "relative",
          zIndex: 1,
        }}
      >
        <div
          style={{
            width: "100%",
            maxWidth: "480px",
            border: "1px solid #555555",
            background: "#222222",
            borderRadius: 0,
            overflow: "hidden",
          }}
        >
          {/* Logo header */}
          <div
            style={{
              padding: "4rem 3rem",
              textAlign: "center",
              borderBottom: "1px solid #555555",
            }}
          >
            <div style={{ marginBottom: "1.5rem" }}>
              <img
                src={resolvedTheme === "dark" ? "/nvroot-dark.svg" : "/nvroot-light.svg"}
                alt="requestrepo"
                style={{
                  width: "320px",
                  height: "auto",
                  maxHeight: "70px",
                  objectFit: "contain",
                }}
              />
            </div>
          </div>

          {/* Login form */}
          <div style={{ padding: "3rem" }}>
            <h2
              style={{
                fontSize: "1.5rem",
                fontWeight: 700,
                textAlign: "center",
                marginBottom: "2rem",
                color: "#ffffff",
              }}
            >
              OOB SERVER
            </h2>

            <div
              style={{
                width: "100%",
                display: "flex",
                flexDirection: "column",
                gap: "1rem",
              }}
            >
              {/* OAuth provider buttons */}
              {showOAuth &&
                providers
                  .filter((p) => p.provider_type !== "password")
                  .map((provider) => (
                    <a
                      key={provider.name}
                      href={provider.login_url}
                      style={{
                        display: "block",
                        width: "100%",
                      }}
                    >
                      <img
                        src={`/${provider.icon}`}
                        alt={`Continue with ${provider.display_name}`}
                        style={{
                          width: "100%",
                          height: "auto",
                          borderRadius: 0,
                          display: "block",
                          cursor: "pointer",
                        }}
                      />
                    </a>
                  ))}

              {/* Password fallback */}
              {!showOAuth && (
                <form onSubmit={handlePasswordLogin}>
                  <div style={{ display: "flex", flexDirection: "column", gap: "1rem" }}>
                    <div className="relative">
                      <Input
                        type={showPassword ? "text" : "password"}
                        placeholder="Enter admin password"
                        value={password}
                        onChange={(v) => {
                          setPassword(v.target.value);
                          if (authError) setAuthError(null);
                        }}
                        autoFocus
                      />
                      <button
                        type="button"
                        onClick={() => setShowPassword(!showPassword)}
                        className="absolute right-2 top-1/2 -translate-y-1/2 focus:outline-hidden"
                      >
                        {showPassword ? (
                          <EyeOff className="h-4 w-4 text-default-400" />
                        ) : (
                          <Eye className="h-4 w-4 text-default-400" />
                        )}
                      </button>
                    </div>

                    {authError && (
                      <div className="flex items-center gap-2 text-danger text-sm">
                        <AlertCircle className="h-4 w-4" />
                        <span>{authError}</span>
                      </div>
                    )}

                    <Button
                      type="submit"
                      variant="primary"
                      isDisabled={!password.trim()}
                      style={{ backgroundColor: "#F6D30F", color: "#000" }}
                    >
                      {isSubmitting ? "Authenticating..." : "Authenticate"}
                    </Button>
                  </div>
                </form>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

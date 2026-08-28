import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { Card, Button, Separator } from "@heroui/react";
import { LogOut, Trash2, Shield } from "lucide-react";
import { toast } from "sonner";

interface UserProfile {
  username: string;
  avatar_url: string;
  name: string;
  created_at: string;
  is_admin: boolean;
}

export function ProfilePage() {
  const navigate = useNavigate();
  const [user, setUser] = useState<UserProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [_logoutLoading, setLogoutLoading] = useState(false);

  useEffect(() => {
    const fetchUser = async () => {
      try {
        const res = await fetch("/auth/user");
        const data = await res.json();
        if (data.authenticated && data.user) {
          setUser(data.user);
        } else {
          navigate("/login");
        }
      } catch {
        navigate("/login");
      } finally {
        setLoading(false);
      }
    };
    fetchUser();
  }, [navigate]);

  const handleLogout = async () => {
    setLogoutLoading(true);
    try {
      window.location.href = "/auth/logout";
    } catch {
      toast.error("Logout failed");
      setLogoutLoading(false);
    }
  };

  const handleLogoutAll = async () => {
    setLogoutLoading(true);
    try {
      window.location.href = "/auth/logout";
    } catch {
      toast.error("Failed to logout all sessions");
      setLogoutLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-default-500">Loading profile...</div>
      </div>
    );
  }

  if (!user) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center">
          <h2 className="text-xl font-bold mb-2">You are not logged in</h2>
          <p className="text-default-500">Please log in to view your profile</p>
        </div>
      </div>
    );
  }

  return (
    <div className="max-w-3xl mx-auto flex flex-col gap-6">
      {/* Profile Info */}
      <Card>
        <Card.Content>
          <div className="flex flex-col md:flex-row gap-6 items-center md:items-start">
            {/* Avatar */}
            <div className="shrink-0">
              {user.avatar_url ? (
                <img
                  src={user.avatar_url}
                  alt="Profile"
                  className="w-40 h-40 rounded-full shadow-lg object-cover"
                />
              ) : (
                <div className="w-40 h-40 rounded-full shadow-lg flex items-center justify-center text-5xl font-bold bg-primary text-white">
                  {(user.name || user.username || "U").charAt(0).toUpperCase()}
                </div>
              )}
            </div>

            {/* Info */}
            <div className="flex-1 text-center md:text-left">
              <div className="text-xl font-bold mb-3">{user.name || user.username}</div>
              <div className="mb-2">
                <span className="font-semibold">Username: </span>
                {user.username}
              </div>
              <div className="mb-2">
                <span className="font-semibold">Account Created: </span>
                {user.created_at
                  ? new Date(user.created_at).toLocaleDateString()
                  : "N/A"}
              </div>
              <div className="mb-2 flex items-center justify-center md:justify-start gap-2">
                <span className="font-semibold">Admin: </span>
                {user.is_admin ? (
                  <span className="inline-flex items-center gap-1 text-green-600">
                    <Shield className="h-4 w-4" /> Yes
                  </span>
                ) : (
                  <span className="text-default-500">No</span>
                )}
              </div>
            </div>
          </div>
        </Card.Content>
      </Card>

      {/* Session Management */}
      <Card>
        <Card.Content className="gap-4 flex flex-col">
          <h3 className="text-lg font-semibold">Session Management</h3>
          <Separator />
          <div className="flex flex-col md:flex-row gap-3">
            <Button
              variant="secondary"
              onPress={handleLogout}
            >
              <LogOut className="h-4 w-4" />
              Logout
            </Button>
            <Button
              variant="danger"
              onPress={handleLogoutAll}
            >
              <Trash2 className="h-4 w-4" />
              Logout All Sessions
            </Button>
          </div>
        </Card.Content>
      </Card>
    </div>
  );
}

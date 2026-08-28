import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Input, Button, Tooltip, Dropdown, Avatar } from "@heroui/react";
import { Star, Plus, Share2, Search, X, Menu, Shield, LogOut, User as UserIcon } from "lucide-react";
import { toast } from "sonner";
import { ThemeToggle } from "./ThemeToggle";
import { useSessionStore } from "@/stores/sessionStore";
import { useRequestStore } from "@/stores/requestStore";
import { useUiStore } from "@/stores/uiStore";
import { useAuthStore } from "@/stores/authStore";
import { apiClient, isAdminRequiredError } from "@/api/client";
import { copyToClipboard } from "@/lib/utils";
import { isProduction } from "@/lib/config";

const MAX_SESSIONS = 5;

export function Topbar() {
  const navigate = useNavigate();
  const sessions = useSessionStore((s) => s.sessions);
  const activeSubdomain = useSessionStore((s) => s.activeSubdomain);
  const addSession = useSessionStore((s) => s.addSession);
  const setActiveSession = useSessionStore((s) => s.setActiveSession);
  const removeSession = useSessionStore((s) => s.removeSession);

  const activeSession = sessions.find((s) => s.subdomain === activeSubdomain);

  const allRequests = useRequestStore((s) => s.requests);

  const visitedRequests = useUiStore((s) => s.visitedRequests);
  const searchQuery = useUiStore((s) => s.searchQuery);
  const setSearchQuery = useUiStore((s) => s.setSearchQuery);
  const toggleSidebar = useUiStore((s) => s.toggleSidebar);

  // Mobile search toggle
  const [searchVisible, setSearchVisible] = useState(false);

  // User profile for dropdown
  const [userProfile, setUserProfile] = useState<{ username: string; name: string; avatar_url: string; is_admin: boolean } | null>(null);

  useEffect(() => {
    fetch("/auth/user")
      .then((r) => r.json())
      .then((data) => {
        if (data.authenticated && data.user) setUserProfile(data.user);
      })
      .catch(() => {});
  }, []);

  // GitHub stars count (only fetch in production)
  const [starsCount, setStarsCount] = useState<number | null>(null);

  useEffect(() => {
    if (!isProduction) return;
    fetch("https://api.github.com/repos/adrgs/requestrepo")
      .then((res) => res.json())
      .then((data) => setStarsCount(data.stargazers_count))
      .catch(() => setStarsCount(null));
  }, []);

  // Get unread count for a specific subdomain
  const getUnreadCount = (subdomain: string) => {
    const requests = allRequests[subdomain] ?? [];
    return requests.filter((r) => !visitedRequests[subdomain]?.[r._id]).length;
  };

  const setShowAuthOverlay = useAuthStore((s) => s.setShowAuthOverlay);

  const handleCreateSession = async () => {
    if (sessions.length >= MAX_SESSIONS) {
      toast.error(`Maximum ${MAX_SESSIONS} sessions allowed`);
      return;
    }
    try {
      const response = await apiClient.createSession();
      addSession({
        subdomain: response.subdomain,
        token: response.token,
        createdAt: new Date().toISOString(),
      });
      toast.success(`Session created: ${response.subdomain}`);
    } catch (error) {
      if (isAdminRequiredError(error)) {
        setShowAuthOverlay(true);
      } else {
        toast.error("Failed to create session");
      }
    }
  };

  const handleShare = () => {
    if (activeSession) {
      const url = `${window.location.origin}/?share=${activeSession.token}`;
      copyToClipboard(url);
      toast.success("Session share link copied to clipboard");
    } else {
      toast.error("No active session to share");
    }
  };

  const handleCloseSession = (e: React.MouseEvent, subdomain: string) => {
    e.stopPropagation();
    if (sessions.length === 1) {
      toast.error("Cannot close the last session");
      return;
    }
    removeSession(subdomain);
    toast.success(`Session ${subdomain} closed`);
  };

  return (
    <div className="flex h-[50px] items-center justify-between bg-white px-3 shadow-xs dark:bg-zinc-800 md:px-5">
      {/* Left: Hamburger + Logo + Star + Badge */}
      <div className="flex min-w-0 flex-1 items-center gap-2 md:gap-4">
        {/* Hamburger menu - mobile only */}
        <Button
          isIconOnly
          variant="ghost"
          size="sm"
          className="rounded-full"
          onPress={toggleSidebar}
        >
          <Menu className="h-5 w-5" />
        </Button>

        <a href="/" className="flex shrink-0 items-center">
          <img src="/logo.svg" alt="requestrepo" className="h-5" />
        </a>

        {isProduction && (
          <Tooltip>
            <Tooltip.Trigger>
              <a
                href="https://github.com/adrgs/requestrepo"
                target="_blank"
                rel="noopener noreferrer"
                className="hidden shrink-0 items-center gap-1.5 rounded-full bg-default-100 px-2.5 py-1 text-xs font-medium transition-colors hover:bg-default-200 md:flex"
              >
                <Star className="h-3.5 w-3.5" />
                {starsCount !== null && <span>{starsCount}</span>}
              </a>
            </Tooltip.Trigger>
            <Tooltip.Content>Star on GitHub</Tooltip.Content>
          </Tooltip>
        )}

        {/* Session Tabs - scrollable on mobile */}
        {sessions.length > 0 && (
          <div className="ml-1 flex min-w-0 flex-1 items-center gap-1 overflow-x-auto md:ml-2">
            {sessions.map((session) => {
              const unreadCount = getUnreadCount(session.subdomain);
              return (
                <div
                  key={session.subdomain}
                  className={`group flex shrink-0 cursor-pointer items-center gap-1.5 rounded-md px-2 py-1 text-xs font-medium transition-colors md:px-3 md:text-sm ${
                    session.subdomain === activeSubdomain
                      ? "bg-primary/10 text-primary"
                      : "bg-default-100 text-default-600 hover:bg-default-200"
                  }`}
                  onClick={() => setActiveSession(session.subdomain)}
                >
                  {unreadCount > 0 && (
                    <span className="flex h-[18px] min-w-[18px] items-center justify-center rounded-full bg-red-500 px-1 text-[10px] font-semibold text-white">
                      {unreadCount}
                    </span>
                  )}
                  <span className="font-mono">{session.subdomain}</span>
                  <button
                    className="ml-1 hidden opacity-0 transition-opacity hover:text-danger group-hover:opacity-100 md:block"
                    onClick={(e) => handleCloseSession(e, session.subdomain)}
                  >
                    <X className="h-3 w-3" />
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Right: Icons + Search */}
      <div className="flex shrink-0 items-center gap-1">
        <Tooltip>
          <Tooltip.Trigger>
            <Button
              isIconOnly
              variant="ghost"
              size="sm"
              className="rounded-full"
              onPress={handleCreateSession}
            >
              <Plus className="h-4 w-4" />
            </Button>
          </Tooltip.Trigger>
          <Tooltip.Content>New session</Tooltip.Content>
        </Tooltip>

        <Tooltip>
          <Tooltip.Trigger>
            <Button
              isIconOnly
              variant="ghost"
              size="sm"
              className="rounded-full"
              onPress={handleShare}
            >
              <Share2 className="h-4 w-4" />
            </Button>
          </Tooltip.Trigger>
          <Tooltip.Content>Share session</Tooltip.Content>
        </Tooltip>

        <ThemeToggle />

        {/* Profile dropdown */}
        {userProfile && (
          <Dropdown>
            <Dropdown.Trigger>
              <Button
                isIconOnly
                variant="ghost"
                size="sm"
                className="rounded-full overflow-hidden"
              >
                {userProfile.avatar_url ? (
                  <Avatar size="sm" className="h-6 w-6">
                    <Avatar.Image src={userProfile.avatar_url} />
                    <Avatar.Fallback />
                  </Avatar>
                ) : (
                  <div className="flex h-6 w-6 items-center justify-center rounded-full bg-primary text-xs font-bold text-black">
                    {(userProfile.name || userProfile.username).charAt(0).toUpperCase()}
                  </div>
                )}
              </Button>
            </Dropdown.Trigger>
            <Dropdown.Popover>
              <Dropdown.Menu aria-label="User menu">
                <Dropdown.Item
                  key="profile"
                  onPress={() => navigate("/profile")}
                >
                  <UserIcon className="h-4 w-4" />
                  Profile
                </Dropdown.Item>
                {userProfile.is_admin && (
                  <Dropdown.Item
                    key="admin"
                    onPress={() => navigate("/admin")}
                  >
                    <Shield className="h-4 w-4" />
                    Admin Dashboard
                  </Dropdown.Item>
                )}
                <Dropdown.Item
                  key="logout"
                  onPress={() => { window.location.href = "/auth/logout"; }}
                >
                  <LogOut className="h-4 w-4" />
                  Logout
                </Dropdown.Item>
              </Dropdown.Menu>
            </Dropdown.Popover>
          </Dropdown>
        )}

        {/* Mobile search toggle */}
        <Button
          isIconOnly
          variant="ghost"
          size="sm"
          className="rounded-full"
          onPress={() => setSearchVisible(!searchVisible)}
        >
          <Search className="h-4 w-4" />
        </Button>

        {/* Desktop search - always visible */}
        <div className="ml-2 hidden w-44 md:block">
          <Input
            placeholder="Search..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="rounded-full"
          />
        </div>
      </div>

      {/* Mobile search bar - expandable */}
      {searchVisible && (
        <div className="absolute left-0 right-0 top-[50px] z-50 bg-white p-2 shadow-md dark:bg-zinc-800 md:hidden">
          <div className="relative">
            <Input
              placeholder="Search requests..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className="w-full rounded-full"
              autoFocus
            />
            <Button
              isIconOnly
              variant="ghost"
              size="sm"
              className="absolute right-0 top-1/2 -translate-y-1/2 rounded-full"
              onPress={() => setSearchVisible(false)}
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

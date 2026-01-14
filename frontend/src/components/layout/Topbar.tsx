import { useEffect, useState } from "react";
import { Input, Button, Tooltip } from "@heroui/react";
import { Star, Plus, Share2, Search, X } from "lucide-react";
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
        // Show auth overlay - user needs to enter admin password
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
    <div className="flex h-[50px] items-center justify-between bg-white px-5 shadow-sm dark:bg-zinc-800">
      {/* Left: Logo + Star + Badge */}
      <div className="flex items-center gap-4">
        <a href="/" className="flex items-center">
          <img src="/logo.svg" alt="requestrepo" className="h-5" />
        </a>

        {isProduction && (
          <Tooltip content="Star on GitHub">
            <a
              href="https://github.com/adrgs/requestrepo"
              target="_blank"
              rel="noopener noreferrer"
              className="flex items-center gap-1.5 rounded-full bg-default-100 px-2.5 py-1 text-xs font-medium hover:bg-default-200 transition-colors"
            >
              <Star className="h-3.5 w-3.5" />
              {starsCount !== null && <span>{starsCount}</span>}
            </a>
          </Tooltip>
        )}

        {/* Session Tabs */}
        {sessions.length > 0 && (
          <div className="flex items-center gap-1 ml-2">
            {sessions.map((session) => {
              const unreadCount = getUnreadCount(session.subdomain);
              return (
                <div
                  key={session.subdomain}
                  className={`group flex items-center gap-1.5 px-3 py-1 rounded-md cursor-pointer text-sm font-medium transition-colors ${
                    session.subdomain === activeSubdomain
                      ? "bg-primary/10 text-primary"
                      : "bg-default-100 text-default-600 hover:bg-default-200"
                  }`}
                  onClick={() => setActiveSession(session.subdomain)}
                >
                  {unreadCount > 0 && (
                    <span className="flex items-center justify-center min-w-[18px] h-[18px] px-1 text-[10px] font-semibold text-white bg-red-500 rounded-full">
                      {unreadCount}
                    </span>
                  )}
                  <span className="font-mono">{session.subdomain}</span>
                  <button
                    className="ml-1 opacity-0 group-hover:opacity-100 hover:text-danger transition-opacity"
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
      <div className="flex items-center gap-1">
        <Tooltip content="New session">
          <Button isIconOnly variant="light" size="sm" radius="full" onPress={handleCreateSession}>
            <Plus className="h-4 w-4" />
          </Button>
        </Tooltip>

        <Tooltip content="Share session">
          <Button isIconOnly variant="light" size="sm" radius="full" onPress={handleShare}>
            <Share2 className="h-4 w-4" />
          </Button>
        </Tooltip>

        <ThemeToggle />

        <Input
          placeholder="Search..."
          size="sm"
          variant="flat"
          value={searchQuery}
          onValueChange={setSearchQuery}
          startContent={<Search className="h-4 w-4 text-default-400" />}
          className="ml-2 w-44"
          radius="full"
          classNames={{
            input: "text-sm",
            inputWrapper: "bg-default-100 shadow-none",
          }}
        />
      </div>
    </div>
  );
}

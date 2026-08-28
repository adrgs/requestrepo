import { useNavigate, useLocation } from "react-router-dom";
import { Button, Input } from "@heroui/react";
import { Download, FileEdit, Network, Bell, Copy, Shuffle } from "lucide-react";
import { toast } from "sonner";
import { useSessionStore } from "@/stores/sessionStore";
import { useAuthStore } from "@/stores/authStore";
import { apiClient, isAdminRequiredError } from "@/api/client";
import { copyToClipboard } from "@/lib/utils";
import { getBaseDomain } from "@/lib/config";

export function Toolbar() {
  const navigate = useNavigate();
  const location = useLocation();
  const sessions = useSessionStore((s) => s.sessions);
  const activeSubdomain = useSessionStore((s) => s.activeSubdomain);
  const replaceSession = useSessionStore((s) => s.replaceSession);

  const setShowAuthOverlay = useAuthStore((s) => s.setShowAuthOverlay);

  const session = sessions.find((s) => s.subdomain === activeSubdomain);
  const currentPath = location.pathname.split("/")[1] || "requests";
  const domain = getBaseDomain();
  const fullDomain = session ? `${session.subdomain}.${domain}` : domain;

  const handleCopyUrl = () => {
    copyToClipboard(`http://${fullDomain}`);
    toast.success("URL copied to clipboard");
  };

  const handleNewUrl = async () => {
    if (!activeSubdomain) {
      toast.error("No active session");
      return;
    }
    try {
      const response = await apiClient.createSession();
      replaceSession(activeSubdomain, {
        subdomain: response.subdomain,
        token: response.token,
        createdAt: new Date().toISOString(),
      });
      toast.success(`New URL: ${response.subdomain}`);
    } catch (error) {
      if (isAdminRequiredError(error)) {
        setShowAuthOverlay(true);
      } else {
        toast.error("Failed to get new URL");
      }
    }
  };

  const tabs = [
    { key: "requests", label: "Requests", icon: Download },
    { key: "response", label: "Response", icon: FileEdit },
    { key: "dns", label: "DNS", icon: Network },
    { key: "notifications", label: "Notifications", icon: Bell },
  ];

  return (
    <div className="flex h-auto min-h-[48px] flex-col gap-2 bg-default-50 px-3 py-2 md:h-12 md:flex-row md:items-center md:justify-between md:gap-0 md:px-6 md:py-0">
      {/* Navigation Tabs */}
      <div className="flex items-center gap-1 rounded-full bg-default-100 p-1">
        {tabs.map((tab) => {
          const Icon = tab.icon;
          const isActive = currentPath === tab.key;
          return (
            <Button
              key={tab.key}
              variant={isActive ? "primary" : "ghost"}
              size="sm"
              className={`rounded-full ${isActive ? "" : "bg-transparent"}`}
              onPress={() => navigate(`/${tab.key}`)}
            >
              <Icon className="h-4 w-4" />
              <span className="hidden md:inline">{tab.label}</span>
            </Button>
          );
        })}
      </div>

      {/* URL and Actions */}
      <div className="flex items-center gap-2">
        <Input
          value={fullDomain}
          readOnly
          className="min-w-0 flex-1 cursor-pointer md:w-56 md:flex-none rounded-lg"
          onClick={() => {
            copyToClipboard(fullDomain);
            toast.success("Domain copied to clipboard");
          }}
        />
        <Button
          variant="primary"
          size="sm"
          className="rounded-lg md:hidden"
          isIconOnly
          onPress={handleCopyUrl}
        >
          <Copy className="h-4 w-4" />
        </Button>
        <Button
          variant="primary"
          size="sm"
          className="rounded-lg hidden md:flex"
          onPress={handleCopyUrl}
        >
          <Copy className="h-4 w-4" />
          Copy URL
        </Button>
        <Button
          variant="primary"
          size="sm"
          className="rounded-lg md:hidden"
          isIconOnly
          onPress={handleNewUrl}
        >
          <Shuffle className="h-4 w-4" />
        </Button>
        <Button
          variant="primary"
          size="sm"
          className="rounded-lg hidden md:flex"
          onPress={handleNewUrl}
        >
          <Shuffle className="h-4 w-4" />
          New URL
        </Button>
      </div>
    </div>
  );
}

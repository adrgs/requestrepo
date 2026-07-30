import { useState, useEffect } from "react";
import {
  Card,
  CardBody,
  CardHeader,
  Button,
  Input,
  Divider,
} from "@heroui/react";
import { Eye, EyeOff, Copy, X, Save, Bell } from "lucide-react";
import { toast } from "sonner";
import { useSessionStore } from "@/stores/sessionStore";
import { apiClient } from "@/api/client";
import { copyToClipboard } from "@/lib/utils";
import type { NotificationSettings } from "@/types";

export function NotificationsPage() {
  const sessions = useSessionStore((s) => s.sessions);
  const activeSubdomain = useSessionStore((s) => s.activeSubdomain);
  const session = sessions.find((s) => s.subdomain === activeSubdomain);

  const [settings, setSettings] = useState<NotificationSettings>({
    discord_webhook_url: "",
    mattermost_webhook_url: "",
    telegram_bot_token: "",
    telegram_chat_id: "",
  });
  const [loading, setLoading] = useState(false);
  const [showDiscord, setShowDiscord] = useState(false);
  const [showMattermost, setShowMattermost] = useState(false);
  const [showTelegramToken, setShowTelegramToken] = useState(false);
  const [showTelegramChat, setShowTelegramChat] = useState(false);

  useEffect(() => {
    if (!session?.token) return;
    const load = async () => {
      try {
        const data = await apiClient.getNotificationSettings(session.token);
        setSettings(data);
      } catch {
        toast.error("Failed to load notification settings");
      }
    };
    load();
  }, [session?.token]);

  const handleSave = async () => {
    if (!session?.token) {
      toast.error("No active session");
      return;
    }
    setLoading(true);
    try {
      await apiClient.updateNotificationSettings(session.token, settings);
      toast.success("Notification settings saved");
    } catch {
      toast.error("Failed to save notification settings");
    } finally {
      setLoading(false);
    }
  };

  const handleTest = async (service: string) => {
    if (!session?.token) {
      toast.error("No active session");
      return;
    }
    setLoading(true);
    try {
      await apiClient.sendTestNotification(session.token, service);
      toast.success(`Test ${service} notification sent`);
    } catch (err) {
      const msg =
        err instanceof Error
          ? err.message
          : `Failed to send test ${service} notification`;
      toast.error(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleCopy = (value: string) => {
    copyToClipboard(value);
    toast.success("Copied to clipboard");
  };

  if (!session) {
    return <div>No session selected</div>;
  }

  const field = (
    label: string,
    id: string,
    value: string,
    onChange: (v: string) => void,
    placeholder: string,
    show: boolean,
    onToggle: () => void,
  ) => (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className="text-sm font-medium text-default-700">
        {label}
      </label>
      <Input
        id={id}
        type={show ? "text" : "password"}
        value={value}
        onValueChange={onChange}
        placeholder={placeholder}
        size="sm"
        endContent={
          <div className="flex items-center gap-1">
            <Button
              isIconOnly
              size="sm"
              variant="light"
              onPress={onToggle}
              className="text-default-400"
            >
              {show ? (
                <EyeOff className="h-4 w-4" />
              ) : (
                <Eye className="h-4 w-4" />
              )}
            </Button>
            <Button
              isIconOnly
              size="sm"
              variant="light"
              onPress={() => handleCopy(value)}
              className="text-default-400"
            >
              <Copy className="h-4 w-4" />
            </Button>
            <Button
              isIconOnly
              size="sm"
              variant="light"
              onPress={() => onChange("")}
              className="text-default-400"
            >
              <X className="h-4 w-4" />
            </Button>
          </div>
        }
      />
    </div>
  );

  return (
    <div className="flex flex-col gap-4">
      <Card>
        <CardBody className="gap-2">
          <div className="flex items-center gap-2">
            <Bell className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">Notification Settings</h2>
          </div>
          <p className="text-sm text-default-500">
            Configure webhook URLs for Discord, Mattermost, and Telegram
            notifications.
          </p>
        </CardBody>
      </Card>

      <Card>
        <CardHeader>
          <h3 className="text-base font-semibold">Webhooks & Tokens</h3>
        </CardHeader>
        <Divider />
        <CardBody className="flex flex-col gap-6">
          {field(
            "Discord Webhook URL",
            "discord-webhook",
            settings.discord_webhook_url,
            (v) => setSettings((s) => ({ ...s, discord_webhook_url: v })),
            "https://discord.com/api/webhooks/...",
            showDiscord,
            () => setShowDiscord(!showDiscord),
          )}

          {field(
            "Mattermost Webhook URL",
            "mattermost-webhook",
            settings.mattermost_webhook_url,
            (v) => setSettings((s) => ({ ...s, mattermost_webhook_url: v })),
            "https://your-mattermost-instance.com/hooks/...",
            showMattermost,
            () => setShowMattermost(!showMattermost),
          )}

          {field(
            "Telegram Bot Token",
            "telegram-bot-token",
            settings.telegram_bot_token,
            (v) => setSettings((s) => ({ ...s, telegram_bot_token: v })),
            "Enter your Telegram bot token",
            showTelegramToken,
            () => setShowTelegramToken(!showTelegramToken),
          )}

          {field(
            "Telegram Chat ID",
            "telegram-chat-id",
            settings.telegram_chat_id,
            (v) => setSettings((s) => ({ ...s, telegram_chat_id: v })),
            "Enter your Telegram chat ID",
            showTelegramChat,
            () => setShowTelegramChat(!showTelegramChat),
          )}

          <Button
            color="primary"
            startContent={<Save className="h-4 w-4" />}
            onPress={handleSave}
            isLoading={loading}
            className="w-full md:w-auto md:self-start"
          >
            Save Settings
          </Button>
        </CardBody>
      </Card>

      <Card>
        <CardHeader>
          <h3 className="text-base font-semibold">Test Notifications</h3>
        </CardHeader>
        <Divider />
        <CardBody>
          <div className="flex flex-wrap gap-3">
            <Button
              startContent={<Bell className="h-4 w-4" />}
              onPress={() => handleTest("discord")}
              isLoading={loading}
              className="text-white"
              style={{ backgroundColor: "#5865F2" }}
            >
              Test Discord
            </Button>
            <Button
              startContent={<Bell className="h-4 w-4" />}
              onPress={() => handleTest("mattermost")}
              isLoading={loading}
              className="text-white"
              style={{ backgroundColor: "#1E325C" }}
            >
              Test Mattermost
            </Button>
            <Button
              startContent={<Bell className="h-4 w-4" />}
              onPress={() => handleTest("telegram")}
              isLoading={loading}
              className="text-white"
              style={{ backgroundColor: "#229ED9" }}
            >
              Test Telegram
            </Button>
          </div>
        </CardBody>
      </Card>
    </div>
  );
}

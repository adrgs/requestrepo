import { Button, Card, CardBody, CardHeader } from "@heroui/react";
import { Plus, Globe } from "lucide-react";
import { toast } from "sonner";
import { apiClient } from "@/api/client";
import { useSessionStore } from "@/stores/sessionStore";
import { useTheme } from "@/hooks/useTheme";
import { ThemeToggle } from "./ThemeToggle";

export function EmptyState() {
  const addSession = useSessionStore((s) => s.addSession);
  const { resolvedTheme } = useTheme();

  const handleCreateSession = async () => {
    try {
      const response = await apiClient.createSession();
      addSession({
        subdomain: response.subdomain,
        token: response.token,
        createdAt: new Date().toISOString(),
      });
      toast.success(`Session created: ${response.subdomain}`);
    } catch (error) {
      toast.error("Failed to create session");
      console.error(error);
    }
  };

  return (
    <div
      className={`${resolvedTheme} flex h-full items-center justify-center bg-background p-8`}
    >
      <Card className="max-w-md">
        <CardHeader className="flex flex-col items-center gap-2 pb-0">
          <Globe className="h-16 w-16 text-primary" />
          <h1 className="text-2xl font-bold">RequestRepo</h1>
          <p className="text-center text-default-500">
            Capture and inspect HTTP and DNS requests in real-time
          </p>
        </CardHeader>
        <CardBody className="items-center gap-4">
          <Button
            color="primary"
            size="lg"
            startContent={<Plus className="h-5 w-5" />}
            onPress={handleCreateSession}
          >
            Create New Session
          </Button>
          <div className="flex items-center gap-2 text-sm text-default-400">
            <span>Theme:</span>
            <ThemeToggle />
          </div>
        </CardBody>
      </Card>
    </div>
  );
}

import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  Button,
} from "@heroui/react";
import { WifiOff, RefreshCw } from "lucide-react";
import { useState } from "react";
import { useAuthStore } from "@/stores/authStore";
import { apiClient, isAdminRequiredError } from "@/api/client";
import { useSessionStore } from "@/stores/sessionStore";

export function BackendOfflineOverlay() {
  const [isRetrying, setIsRetrying] = useState(false);
  const setBackendOffline = useAuthStore((s) => s.setBackendOffline);
  const setShowAuthOverlay = useAuthStore((s) => s.setShowAuthOverlay);
  const addSession = useSessionStore((s) => s.addSession);

  const handleRetry = async () => {
    setIsRetrying(true);
    try {
      const response = await apiClient.createSession();
      setBackendOffline(false);
      addSession({
        subdomain: response.subdomain,
        token: response.token,
        createdAt: new Date().toISOString(),
      });
    } catch (error) {
      if (isAdminRequiredError(error)) {
        setBackendOffline(false);
        setShowAuthOverlay(true);
      }
      // Still offline, keep showing this overlay
    } finally {
      setIsRetrying(false);
    }
  };

  return (
    <Modal
      isOpen={true}
      hideCloseButton
      isDismissable={false}
      backdrop="blur"
      classNames={{
        backdrop: "bg-black/80",
      }}
    >
      <ModalContent>
        <ModalHeader className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <WifiOff className="h-5 w-5 text-danger" />
            <span>Cannot Connect to Backend</span>
          </div>
        </ModalHeader>
        <ModalBody className="pb-6">
          <p className="text-sm text-default-500 mb-4">
            Unable to connect to the RequestRepo backend service. This could be
            a temporary network issue or the service may be unavailable.
          </p>

          <Button
            color="primary"
            onPress={handleRetry}
            isLoading={isRetrying}
            startContent={!isRetrying && <RefreshCw className="h-4 w-4" />}
            className="w-full"
          >
            {isRetrying ? "Connecting..." : "Retry Connection"}
          </Button>
        </ModalBody>
      </ModalContent>
    </Modal>
  );
}

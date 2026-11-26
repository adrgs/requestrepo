import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  Button,
} from "@heroui/react";
import { Ban, Shuffle } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { useAuthStore } from "@/stores/authStore";
import { useBlockedStore } from "@/stores/blockedStore";
import { useSessionStore } from "@/stores/sessionStore";
import { apiClient, isAdminRequiredError } from "@/api/client";
import { getAbuseContact } from "@/lib/config";

interface BlockedSubdomainOverlayProps {
  subdomain: string;
}

/**
 * Shown when the active session has been suspended for abuse. Nothing in the dashboard
 * works for it any more, so the only ways forward are a new URL or another session.
 */
export function BlockedSubdomainOverlay({
  subdomain,
}: BlockedSubdomainOverlayProps) {
  const [isReplacing, setIsReplacing] = useState(false);
  const sessions = useSessionStore((s) => s.sessions);
  const replaceSession = useSessionStore((s) => s.replaceSession);
  const setActiveSession = useSessionStore((s) => s.setActiveSession);
  const removeSession = useSessionStore((s) => s.removeSession);
  const blocked = useBlockedStore((s) => s.blocked);
  const setShowAuthOverlay = useAuthStore((s) => s.setShowAuthOverlay);
  const contact = getAbuseContact();

  const alternative = sessions.find(
    (session) => session.subdomain !== subdomain && !blocked[session.subdomain],
  );

  const handleNewUrl = async () => {
    setIsReplacing(true);
    try {
      const response = await apiClient.createSession();
      replaceSession(subdomain, {
        subdomain: response.subdomain,
        token: response.token,
        createdAt: new Date().toISOString(),
      });
    } catch (error) {
      if (isAdminRequiredError(error)) {
        setShowAuthOverlay(true);
      } else {
        toast.error("Failed to get a new URL");
      }
    } finally {
      setIsReplacing(false);
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
            <Ban className="h-5 w-5 text-danger" />
            <span>This session has been suspended</span>
          </div>
        </ModalHeader>
        <ModalBody className="pb-6">
          <p className="text-sm text-default-500">
            <span className="font-mono text-foreground">{subdomain}</span> was
            reported for abuse and suspended. It no longer serves responses or
            resolves, and new requests to it are not shown here.
          </p>
          <p className="text-sm text-default-500">
            If you were using it for legitimate testing, take a new URL and
            carry on.
            {contact ? (
              <>
                {" "}
                To dispute the suspension, contact{" "}
                <span className="text-foreground">{contact}</span>.
              </>
            ) : null}
          </p>

          <div className="mt-4 flex flex-col gap-2">
            <Button
              color="primary"
              isLoading={isReplacing}
              startContent={!isReplacing && <Shuffle className="h-4 w-4" />}
              onPress={handleNewUrl}
              className="w-full"
            >
              Get a new URL
            </Button>
            {alternative && (
              <>
                <Button
                  variant="flat"
                  onPress={() => setActiveSession(alternative.subdomain)}
                  className="w-full"
                >
                  Switch to{" "}
                  <span className="font-mono">{alternative.subdomain}</span>
                </Button>
                <Button
                  variant="light"
                  onPress={() => removeSession(subdomain)}
                  className="w-full"
                >
                  Close this session
                </Button>
              </>
            )}
          </div>
        </ModalBody>
      </ModalContent>
    </Modal>
  );
}

import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { AxiosError } from "axios";
import {
  Button,
  Input,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
} from "@heroui/react";
import { CircleCheck, Flag } from "lucide-react";
import { apiClient } from "@/api/client";
import { extractSubdomain } from "@/lib/abuse";
import { getDnsDomain, isAbuseReportingEnabled } from "@/lib/config";

function describeError(error: unknown): string {
  if (error instanceof AxiosError && error.response) {
    const data = error.response.data as
      | { error?: string; code?: string; retry_after?: number }
      | undefined;
    if (data?.code === "rate_limited") {
      const minutes = Math.max(1, Math.ceil((data.retry_after ?? 3600) / 60));
      return `Too many reports from your address. Try again in about ${minutes} minute${minutes === 1 ? "" : "s"}.`;
    }
    if (data?.code === "invalid_subdomain") {
      return `That is not a subdomain of ${getDnsDomain()}.`;
    }
    if (data?.code === "admin_required") {
      return "This instance is private: sign in on the dashboard first.";
    }
    if (data?.error) return data.error;
  }
  return "The report could not be sent. Please try again.";
}

interface ReportAbuseDialogProps {
  isOpen: boolean;
  onClose: () => void;
  // Prefills the subdomain, e.g. from /abuse?subdomain=...
  initialSubdomain?: string;
  // Whether the dialog can be dismissed (it cannot when it is the whole page)
  isDismissable?: boolean;
}

/** Reports a subdomain for abuse. A report suspends it immediately. */
export function ReportAbuseDialog({
  isOpen,
  onClose,
  initialSubdomain = "",
  isDismissable = true,
}: ReportAbuseDialogProps) {
  const domain = getDnsDomain();
  const enabled = isAbuseReportingEnabled();
  const [subdomain, setSubdomain] = useState(initialSubdomain);

  const report = useMutation({
    mutationFn: () => apiClient.createAbuseReport({ subdomain }),
  });

  const label = extractSubdomain(subdomain);
  const reset = () => {
    report.reset();
    setSubdomain("");
  };
  const close = () => {
    reset();
    onClose();
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={close}
      size="md"
      placement="center"
      hideCloseButton={!isDismissable}
      isDismissable={isDismissable}
      isKeyboardDismissDisabled={!isDismissable}
    >
      <ModalContent>
        {report.data ? (
          <>
            <ModalBody className="items-center gap-2 pb-2 pt-8 text-center">
              <CircleCheck className="h-10 w-10 text-success" />
              <h2 className="text-lg font-semibold">Subdomain suspended</h2>
              <p className="text-sm text-default-500">
                <span className="break-all font-mono text-foreground">
                  {report.data.subdomain}.{domain}
                </span>{" "}
                {report.data.already_suspended
                  ? "was already suspended."
                  : "no longer serves content or resolves."}{" "}
                Thank you for the report.
              </p>
            </ModalBody>
            <ModalFooter className="justify-center">
              <Button variant="flat" onPress={reset}>
                Report another
              </Button>
              {isDismissable && (
                <Button color="primary" onPress={close}>
                  Done
                </Button>
              )}
            </ModalFooter>
          </>
        ) : (
          <>
            <ModalHeader className="flex items-center gap-2">
              <Flag className="h-5 w-5 text-danger" />
              Report abuse
            </ModalHeader>
            {enabled ? (
              <>
                <ModalBody className="gap-4">
                  <p className="text-sm text-default-500">
                    Report a subdomain of{" "}
                    <span className="font-mono text-foreground">{domain}</span>{" "}
                    used for malware, phishing or other abuse. It is suspended
                    immediately.
                  </p>
                  <Input
                    autoFocus
                    aria-label="Subdomain"
                    placeholder={`abcd1234.${domain}`}
                    description={
                      label && `${label}.${domain}` !== subdomain.trim()
                        ? `${label}.${domain}`
                        : "A name or a URL"
                    }
                    value={subdomain}
                    onValueChange={setSubdomain}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" && label && !report.isPending) {
                        report.mutate();
                      }
                    }}
                    // ARIA validation: the native kind keeps an old "invalid" state
                    // after the value becomes valid
                    validationBehavior="aria"
                    isInvalid={subdomain.trim() !== "" && !label}
                    errorMessage={`Not a subdomain of ${domain}`}
                    autoComplete="off"
                    classNames={{ input: "font-mono" }}
                  />
                  {report.error && (
                    <p className="text-sm text-danger" role="alert">
                      {describeError(report.error)}
                    </p>
                  )}
                </ModalBody>
                <ModalFooter>
                  {isDismissable && (
                    <Button variant="light" onPress={close}>
                      Cancel
                    </Button>
                  )}
                  <Button
                    color="danger"
                    isLoading={report.isPending}
                    isDisabled={!label}
                    onPress={() => report.mutate()}
                  >
                    Report and suspend
                  </Button>
                </ModalFooter>
              </>
            ) : (
              <ModalBody className="pb-6">
                <p className="text-sm text-default-500">
                  Abuse reporting is turned off on this instance.
                </p>
              </ModalBody>
            )}
          </>
        )}
      </ModalContent>
    </Modal>
  );
}

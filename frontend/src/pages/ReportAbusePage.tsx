import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { AxiosError } from "axios";
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  Checkbox,
  Input,
  Select,
  SelectItem,
  Textarea,
} from "@heroui/react";
import { CheckCircle2, Flag, ShieldAlert } from "lucide-react";
import { apiClient } from "@/api/client";
import { StandaloneLayout } from "@/components/layout/StandaloneLayout";
import { usePageTitle } from "@/hooks/usePageTitle";
import { ABUSE_CATEGORIES, extractSubdomain } from "@/lib/abuse";
import {
  getAbuseContact,
  getDnsDomain,
  isAbuseReportingEnabled,
} from "@/lib/config";
import type { AbuseCategory } from "@/types";

// The same limits the server enforces
const DESCRIPTION_MAX = 1000;
const EVIDENCE_URL_MAX = 500;
const CONTACT_MAX = 200;

// A full web address. The server checks the same thing; this only says so at once.
const WEB_ADDRESS = /^https?:\/\/\S+$/i;

function describeError(error: unknown): string {
  if (error instanceof AxiosError && error.response) {
    const data = error.response.data as
      | { error?: string; retry_after?: number }
      | undefined;
    if (error.response.status === 429) {
      const minutes = Math.max(1, Math.ceil((data?.retry_after ?? 3600) / 60));
      return `Too many reports from your network. Try again in about ${minutes} minute${minutes === 1 ? "" : "s"}.`;
    }
    if (data?.error) return data.error;
  }
  return "The report could not be submitted. Please try again later.";
}

function ReportForm() {
  const domain = getDnsDomain();
  const contact = getAbuseContact();
  // A link may name the subdomain to report: /abuse?subdomain=cqzbr8a4
  const [subdomain, setSubdomain] = useState(
    () => new URLSearchParams(window.location.search).get("subdomain") ?? "",
  );
  const [category, setCategory] = useState<AbuseCategory>("malware");
  const [description, setDescription] = useState("");
  const [evidenceUrl, setEvidenceUrl] = useState("");
  const [contactField, setContactField] = useState("");
  const [confirmed, setConfirmed] = useState(false);

  const mutation = useMutation({
    mutationFn: () =>
      apiClient.createAbuseReport({
        subdomain,
        category,
        description,
        evidence_url: evidenceUrl.trim() || undefined,
        contact: contactField.trim() || undefined,
      }),
  });

  const parsed = extractSubdomain(subdomain);
  // Spell the name out only when that adds something: a bare label, a URL, another case
  const hostname = parsed ? `${parsed}.${domain}` : null;
  const subdomainHint =
    hostname && hostname !== subdomain.trim().toLowerCase()
      ? `Reporting ${hostname}`
      : "A label, a hostname or a full URL";
  const evidence = evidenceUrl.trim();
  const evidenceInvalid = evidence !== "" && !WEB_ADDRESS.test(evidence);
  const result = mutation.data;
  const error = mutation.error ? describeError(mutation.error) : null;

  if (result) {
    return (
      <Card>
        <CardBody className="items-center gap-3 py-8 text-center">
          <CheckCircle2 className="h-10 w-10 text-success" />
          <h2 className="text-xl font-semibold">
            {result.status === "blocked"
              ? "Subdomain blocked"
              : "Report recorded"}
          </h2>
          <p className="max-w-md text-sm text-default-500">
            {result.status === "blocked" ? (
              <>
                <span className="font-mono text-foreground">
                  {result.subdomain}.{domain}
                </span>{" "}
                is now blocked. Requests to it are refused, its name no longer
                resolves and its owner can no longer use it.
              </>
            ) : (
              <>
                <span className="font-mono text-foreground">
                  {result.subdomain}.{domain}
                </span>{" "}
                was already blocked. Your report has been added to it.
              </>
            )}
          </p>
          <Button
            variant="flat"
            className="mt-2"
            onPress={() => {
              mutation.reset();
              setDescription("");
              setEvidenceUrl("");
              setContactField("");
              setConfirmed(false);
              setSubdomain("");
            }}
          >
            Report another subdomain
          </Button>
        </CardBody>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader className="flex flex-col items-start gap-1.5">
        <h2 className="text-lg font-semibold">Report a subdomain</h2>
        <p className="text-sm text-default-500">
          Fields marked optional can be left empty. Everything else is required.
        </p>
      </CardHeader>
      <CardBody className="gap-4">
        <Input
          label="Subdomain"
          placeholder={`name.${domain}`}
          description={subdomainHint}
          value={subdomain}
          onValueChange={setSubdomain}
          isRequired
          autoComplete="off"
          classNames={{ input: "font-mono" }}
        />

        <Select
          label="Category"
          selectedKeys={[category]}
          onSelectionChange={(keys) => {
            const key = Array.from(keys)[0] as AbuseCategory | undefined;
            if (key) setCategory(key);
          }}
          isRequired
        >
          {ABUSE_CATEGORIES.map((option) => (
            <SelectItem key={option.key}>{option.label}</SelectItem>
          ))}
        </Select>

        <Textarea
          label="What is the subdomain being used for?"
          placeholder="For example: the loader of a miner dropper requests it on every infected host"
          description={`${description.trim().length}/${DESCRIPTION_MAX}`}
          value={description}
          onValueChange={setDescription}
          maxLength={DESCRIPTION_MAX}
          isRequired
          classNames={{ input: "min-h-24" }}
        />

        {/* Plain text inputs on purpose. With `type="url"` or `type="email"` the browser
            flags anything else with a validation bubble of its own, even though both
            fields are optional, and a contact is as likely to be a handle as an address. */}
        <Input
          label="Evidence URL (optional)"
          placeholder="https://threatfox.abuse.ch/ioc/…"
          description="A page that documents the abuse, such as a threat-intelligence entry"
          value={evidenceUrl}
          onValueChange={setEvidenceUrl}
          maxLength={EVIDENCE_URL_MAX}
          autoComplete="off"
          isInvalid={evidenceInvalid}
          errorMessage="Enter a full web address that starts with http:// or https://"
        />
        <Input
          label="Contact (optional)"
          placeholder="Email or handle, in case the operator needs to ask about this report"
          value={contactField}
          onValueChange={setContactField}
          maxLength={CONTACT_MAX}
          autoComplete="off"
        />

        <Checkbox
          isSelected={confirmed}
          onValueChange={setConfirmed}
          size="sm"
          classNames={{ label: "text-sm text-default-500" }}
        >
          I have seen this subdomain in abusive traffic and understand that
          reporting it blocks it immediately.
        </Checkbox>

        {error && (
          <p className="text-sm text-danger" role="alert">
            {error}
          </p>
        )}

        <Button
          color="primary"
          size="md"
          startContent={<Flag className="h-4 w-4" />}
          isLoading={mutation.isPending}
          isDisabled={
            !parsed ||
            !description.trim() ||
            evidenceInvalid ||
            !confirmed ||
            mutation.isPending
          }
          onPress={() => mutation.mutate()}
          className="w-full sm:w-auto sm:self-start"
        >
          Submit report
        </Button>

        {contact && (
          <p className="text-xs text-default-400">
            Prefer email? Contact{" "}
            <a
              className="text-primary hover:underline"
              href={`mailto:${contact}`}
            >
              {contact}
            </a>
          </p>
        )}
      </CardBody>
    </Card>
  );
}

/**
 * Public page for reporting a subdomain that is being abused. A report blocks it.
 * Lives outside the dashboard: a visitor reporting abuse should not be handed a session.
 */
export function ReportAbusePage() {
  usePageTitle("Report abuse");
  const enabled = isAbuseReportingEnabled();
  const contact = getAbuseContact();

  return (
    <StandaloneLayout>
      <Card>
        <CardBody className="gap-2">
          <div className="flex items-center gap-2">
            <ShieldAlert className="h-5 w-5 text-primary" />
            <h1 className="text-xl font-semibold">Report abuse</h1>
          </div>
          <p className="text-sm text-default-500">
            Report a subdomain of{" "}
            <span className="font-mono text-foreground">{getDnsDomain()}</span>{" "}
            that is being used for abuse, for example as a malware beacon or to
            host phishing pages. A report blocks the subdomain at once: requests
            to it are refused, its name stops resolving and its owner can no
            longer use it.
          </p>
          <p className="text-sm text-default-500">
            Subdomain names are random, so only report one you have actually
            seen in traffic. Each network can file up to 3 reports per hour.
          </p>
        </CardBody>
      </Card>

      {enabled ? (
        <ReportForm />
      ) : (
        <Card>
          <CardBody>
            <p className="text-sm text-default-500">
              Abuse reporting is turned off on this instance.
              {contact && ` To report a subdomain, contact ${contact}.`}
            </p>
          </CardBody>
        </Card>
      )}

      <p className="text-center text-xs text-default-400">
        A block stays in place until the operator of this instance lifts it.
      </p>
    </StandaloneLayout>
  );
}

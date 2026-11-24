import { useDeferredValue, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  Chip,
  Input,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
  Select,
  SelectItem,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
} from "@heroui/react";
import { Ban, Download, Lock, ShieldOff, Trash2 } from "lucide-react";
import { toast } from "sonner";
import {
  adminAccessError,
  apiClient,
  isAdminRequiredError,
} from "@/api/client";
import { AdminAuthOverlay } from "@/components/auth/AdminAuthOverlay";
import { StandaloneLayout } from "@/components/layout/StandaloneLayout";
import { usePageTitle } from "@/hooks/usePageTitle";
import { useAuthStore } from "@/stores/authStore";
import {
  ABUSE_CATEGORIES,
  categoryLabel,
  downloadJson,
  extractSubdomain,
} from "@/lib/abuse";
import { getDnsDomain } from "@/lib/config";
import { formatDate, getFlagClass } from "@/lib/utils";
import { isDnsRequest, isHttpRequest } from "@/types";
import type { AbuseCategory, Request } from "@/types";

const PAGE_SIZE = 50;

/** One line about a captured request. */
function summarize(request: Request): string {
  if (isHttpRequest(request)) {
    return `${request.method} ${request.path}${request.query ?? ""}`;
  }
  if (isDnsRequest(request)) return `${request.query_type} ${request.domain}`;
  return request.subject ?? request.command;
}

/** A change takes effect even when the server cannot save it. Say so when it could not. */
function warnIfUnsaved(persisted: boolean) {
  if (!persisted) {
    toast.warning(
      "Applied, but not saved: it will be lost when the server restarts.",
    );
  }
}

function BlockForm() {
  const queryClient = useQueryClient();
  const [subdomain, setSubdomain] = useState("");
  const [category, setCategory] = useState<AbuseCategory>("other");
  const [note, setNote] = useState("");
  const label = extractSubdomain(subdomain);

  const block = useMutation({
    mutationFn: () =>
      apiClient.blockAbuseEntry(label ?? "", {
        category,
        note: note.trim() || undefined,
      }),
    onSuccess: (result) => {
      warnIfUnsaved(result.persisted);
      toast.success(
        result.status === "blocked"
          ? `${result.subdomain} is now blocked`
          : `${result.subdomain} was already blocked`,
      );
      setSubdomain("");
      setNote("");
      queryClient.invalidateQueries({ queryKey: ["abuse"] });
    },
    onError: () => toast.error("Failed to block the subdomain"),
  });

  return (
    <Card>
      <CardHeader>
        <h3 className="text-base font-semibold">Block a subdomain directly</h3>
      </CardHeader>
      <CardBody className="gap-3">
        <div className="flex flex-col gap-3 md:flex-row">
          <Input
            label="Subdomain"
            placeholder={`name.${getDnsDomain()}`}
            value={subdomain}
            onValueChange={setSubdomain}
            isRequired
            autoComplete="off"
            classNames={{ input: "font-mono" }}
            className="md:w-72"
          />
          <Select
            label="Category"
            selectedKeys={[category]}
            onSelectionChange={(keys) => {
              const key = Array.from(keys)[0] as AbuseCategory | undefined;
              if (key) setCategory(key);
            }}
            className="md:w-60"
          >
            {ABUSE_CATEGORIES.map((option) => (
              <SelectItem key={option.key}>{option.label}</SelectItem>
            ))}
          </Select>
          <Input
            label="Note (optional)"
            value={note}
            onValueChange={setNote}
            maxLength={500}
            className="md:flex-1"
          />
        </div>
        <Button
          color="danger"
          variant="flat"
          startContent={<Ban className="h-4 w-4" />}
          isLoading={block.isPending}
          isDisabled={!label || block.isPending}
          onPress={() => block.mutate()}
          className="w-full sm:w-auto sm:self-start"
        >
          Block subdomain
        </Button>
      </CardBody>
    </Card>
  );
}

interface EntryDialogProps {
  subdomain: string;
  onClose: () => void;
  // Called once the block is lifted, so the list can drop back to a page that exists
  onLifted: () => void;
}

/** One blocked subdomain: why it was blocked, what it captured, and what to do with it. */
function EntryDialog({ subdomain, onClose, onLifted }: EntryDialogProps) {
  const queryClient = useQueryClient();
  const [confirming, setConfirming] = useState<"lift" | "purge" | null>(null);

  const entry = useQuery({
    queryKey: ["abuse", "entry", subdomain],
    queryFn: () => apiClient.getAbuseEntry(subdomain),
  });
  const requests = useQuery({
    queryKey: ["abuse", "entry-requests", subdomain],
    queryFn: () => apiClient.getAbuseEntryRequests(subdomain),
  });

  const lift = useMutation({
    mutationFn: () => apiClient.liftAbuseEntry(subdomain),
    onSuccess: ({ persisted }) => {
      warnIfUnsaved(persisted);
      toast.success(`The block on ${subdomain} is lifted`);
      queryClient.invalidateQueries({ queryKey: ["abuse"] });
      onLifted();
      onClose();
    },
    onError: () => toast.error("Failed to lift the block"),
  });
  const purge = useMutation({
    mutationFn: () => apiClient.purgeAbuseEntryData(subdomain),
    onSuccess: () => {
      toast.success(`Deleted the data of ${subdomain}`);
      setConfirming(null);
      queryClient.invalidateQueries({ queryKey: ["abuse"] });
    },
    onError: () => toast.error("Failed to delete the data"),
  });

  const captured = requests.data?.requests ?? [];

  return (
    <Modal
      isOpen={true}
      onClose={onClose}
      size="4xl"
      scrollBehavior="inside"
      classNames={{
        wrapper: "z-50",
      }}
    >
      <ModalContent>
        <ModalHeader className="flex flex-wrap items-center gap-2">
          <span className="font-mono">
            {subdomain}.{getDnsDomain()}
          </span>
          {entry.data && (
            <>
              <Chip size="sm" variant="flat" color="danger">
                {categoryLabel(entry.data.category)}
              </Chip>
              <Chip size="sm" variant="flat">
                {entry.data.source === "admin"
                  ? "Blocked by an admin"
                  : "Blocked by a report"}
              </Chip>
              <span className="text-sm font-normal text-default-500">
                since {formatDate(entry.data.created_at)}
              </span>
            </>
          )}
        </ModalHeader>

        <ModalBody className="gap-5">
          {/* Everything below is text supplied by whoever reported the subdomain, or
              captured from it. It is rendered as text and never as markup. */}
          <section className="flex flex-col gap-3">
            <h3 className="text-sm font-semibold">Why it was blocked</h3>
            {entry.data?.admin_note && (
              <p className="text-sm">{entry.data.admin_note}</p>
            )}
            {entry.data && entry.data.reports.length === 0 && (
              <p className="text-sm text-default-400">
                No reports were recorded.
              </p>
            )}
            {entry.data?.reports.map((report, index) => (
              <div
                key={index}
                className="flex flex-col gap-1 rounded-lg bg-default-100 p-3"
              >
                <div className="flex flex-wrap items-center gap-2 text-xs text-default-500">
                  <span className="font-mono">{report.network}</span>
                  <span>{formatDate(report.at)}</span>
                  <Chip size="sm" variant="flat">
                    {categoryLabel(report.category)}
                  </Chip>
                  {report.contact && <span>contact: {report.contact}</span>}
                </div>
                <p className="whitespace-pre-wrap break-words text-sm">
                  {report.description}
                </p>
                {report.evidence_url &&
                  (/^https?:\/\//i.test(report.evidence_url) ? (
                    <a
                      href={report.evidence_url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="break-all text-sm text-primary hover:underline"
                    >
                      {report.evidence_url}
                    </a>
                  ) : (
                    <span className="break-all text-sm text-default-500">
                      {report.evidence_url}
                    </span>
                  ))}
              </div>
            ))}
          </section>

          <section className="flex flex-col gap-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <h3 className="text-sm font-semibold">
                Captured before the block
              </h3>
              <Button
                size="sm"
                variant="flat"
                startContent={<Download className="h-4 w-4" />}
                isDisabled={captured.length === 0}
                onPress={() =>
                  downloadJson(`${subdomain}-requests.json`, captured)
                }
              >
                Export JSON
              </Button>
            </div>
            <p className="text-xs text-default-400">
              Only what the server still holds. Requests are kept in memory and
              do not survive a restart.
            </p>
            <Table
              aria-label="Captured requests"
              isCompact
              removeWrapper
              classNames={{ wrapper: "max-h-72 overflow-auto" }}
            >
              <TableHeader>
                <TableColumn key="time">Time</TableColumn>
                <TableColumn key="type">Type</TableColumn>
                <TableColumn key="source">Source</TableColumn>
                <TableColumn key="request">Request</TableColumn>
              </TableHeader>
              <TableBody
                items={captured}
                isLoading={requests.isLoading}
                loadingContent="Loading…"
                emptyContent="Nothing was captured"
              >
                {(request) => (
                  <TableRow key={request._id}>
                    <TableCell className="text-xs">
                      {formatDate(request.date)}
                    </TableCell>
                    <TableCell>
                      <Chip size="sm" variant="flat">
                        {request.type.toUpperCase()}
                      </Chip>
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      <span
                        className={`${getFlagClass(request.country)} mr-1`}
                      />
                      {request.ip}
                    </TableCell>
                    <TableCell className="break-all font-mono text-xs">
                      {summarize(request)}
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </section>

          {confirming && (
            <p
              role="alert"
              className="rounded-lg bg-default-100 p-3 text-sm text-foreground"
            >
              {confirming === "lift"
                ? `${subdomain} starts working again right away, for whoever holds its token. A new report can block it again.`
                : `This deletes the files, DNS records and captured requests of ${subdomain}. It cannot be undone. The subdomain stays blocked.`}
            </p>
          )}
        </ModalBody>

        <ModalFooter>
          {confirming === "purge" ? (
            <>
              <Button variant="light" onPress={() => setConfirming(null)}>
                Cancel
              </Button>
              <Button
                color="danger"
                isLoading={purge.isPending}
                onPress={() => purge.mutate()}
              >
                Delete the data
              </Button>
            </>
          ) : confirming === "lift" ? (
            <>
              <Button variant="light" onPress={() => setConfirming(null)}>
                Cancel
              </Button>
              <Button
                color="primary"
                isLoading={lift.isPending}
                onPress={() => lift.mutate()}
              >
                Lift the block
              </Button>
            </>
          ) : (
            <>
              <Button
                color="danger"
                variant="flat"
                startContent={<Trash2 className="h-4 w-4" />}
                isDisabled={purge.isPending}
                onPress={() => setConfirming("purge")}
              >
                Purge data…
              </Button>
              <Button
                color="primary"
                variant="flat"
                startContent={<ShieldOff className="h-4 w-4" />}
                isDisabled={lift.isPending}
                onPress={() => setConfirming("lift")}
              >
                Lift block…
              </Button>
            </>
          )}
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

/**
 * Lists the blocked subdomains for whoever holds the admin token, and lets them block,
 * lift and purge. Outside the dashboard: it creates no session and opens no WebSocket.
 */
export function AbuseAdminPage() {
  usePageTitle("Blocked subdomains");
  const queryClient = useQueryClient();
  const setAuthError = useAuthStore((s) => s.setAuthError);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);

  const entries = useQuery({
    queryKey: ["abuse", "entries", deferredSearch, offset],
    queryFn: () =>
      apiClient.getAbuseEntries({
        q: deferredSearch.trim() || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    // An access problem is not worth retrying, and a retry would only delay the prompt
    retry: (failureCount, error) =>
      adminAccessError(error) === null && failureCount < 1,
  });

  const access = entries.error ? adminAccessError(entries.error) : null;
  const page = entries.data;

  const handleAuth = async (password: string) => {
    try {
      // There is no separate sign-in: sending the admin password with a session request
      // is what sets the cookie. The session it creates is not used.
      await apiClient.createSession(password);
      setAuthError(null);
      await queryClient.invalidateQueries({ queryKey: ["abuse"] });
    } catch (error) {
      if (isAdminRequiredError(error)) {
        setAuthError("Invalid admin password");
      } else {
        setAuthError("Failed to authenticate. Please try again.");
      }
    }
  };

  const hasNext = page ? offset + page.entries.length < page.total : false;

  return (
    <StandaloneLayout>
      {access === "unauthorized" && (
        <AdminAuthOverlay
          title="Admin access required"
          description="Enter the admin password to manage blocked subdomains."
          onSubmit={handleAuth}
        />
      )}

      <Card>
        <CardBody className="gap-2">
          <div className="flex items-center gap-2">
            <Ban className="h-5 w-5 text-danger" />
            <h1 className="text-xl font-semibold">Blocked subdomains</h1>
          </div>
          <p className="text-sm text-default-500">
            A blocked subdomain is refused everywhere: its requests get{" "}
            <code>410 Gone</code>, its name does not resolve and its
            owner&apos;s token stops working. Blocks are saved and survive a
            restart.
          </p>
        </CardBody>
      </Card>

      {access === "not_configured" ? (
        <Card>
          <CardBody className="flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <Lock className="h-5 w-5 text-default-400" />
              <h2 className="text-base font-semibold">
                Not available on this instance
              </h2>
            </div>
            <p className="text-sm text-default-500">
              Managing blocked subdomains requires an admin token. None is
              configured here, so a block can only be lifted by removing its
              entry from <code>abuse.json</code> in the server&apos;s data
              directory and restarting.
            </p>
          </CardBody>
        </Card>
      ) : access === null ? (
        <>
          <BlockForm />

          <Card>
            <CardHeader className="flex flex-col gap-3">
              <h3 className="text-base font-semibold">Blocked subdomains</h3>
              <Input
                placeholder="Search by subdomain"
                aria-label="Search by subdomain"
                value={search}
                onValueChange={(value) => {
                  setSearch(value);
                  setOffset(0);
                }}
                isClearable
                onClear={() => setOffset(0)}
                className="max-w-xs"
              />
            </CardHeader>
            <CardBody>
              <Table
                aria-label="Blocked subdomains"
                selectionMode="none"
                onRowAction={(key) => setSelected(String(key))}
                removeWrapper
              >
                <TableHeader>
                  <TableColumn key="subdomain">Subdomain</TableColumn>
                  <TableColumn key="category">Category</TableColumn>
                  <TableColumn key="origin">Origin</TableColumn>
                  <TableColumn key="reports">Reports</TableColumn>
                  <TableColumn key="since">Blocked since</TableColumn>
                </TableHeader>
                <TableBody
                  isLoading={entries.isLoading}
                  loadingContent="Loading…"
                  items={page?.entries ?? []}
                  emptyContent={
                    entries.error
                      ? "Could not load the blocklist"
                      : "No blocked subdomains"
                  }
                >
                  {(entry) => (
                    <TableRow key={entry.subdomain} className="cursor-pointer">
                      <TableCell className="font-mono text-sm">
                        {entry.subdomain}
                      </TableCell>
                      <TableCell>
                        <Chip size="sm" variant="flat" color="danger">
                          {categoryLabel(entry.category)}
                        </Chip>
                      </TableCell>
                      <TableCell className="text-sm text-default-500">
                        {entry.source === "admin" ? "Admin" : "Report"}
                      </TableCell>
                      <TableCell className="text-sm">
                        {entry.report_count}
                      </TableCell>
                      <TableCell className="text-sm text-default-500">
                        {formatDate(entry.created_at)}
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>

              {page && page.entries.length > 0 && (
                <div className="mt-3 flex flex-wrap items-center justify-between gap-2">
                  <span className="text-sm text-default-500">
                    {offset + 1}–
                    {Math.min(offset + page.entries.length, page.total)} of{" "}
                    {page.total}
                  </span>
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="flat"
                      isDisabled={offset === 0}
                      onPress={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
                    >
                      Previous
                    </Button>
                    <Button
                      size="sm"
                      variant="flat"
                      isDisabled={!hasNext}
                      onPress={() => setOffset(offset + PAGE_SIZE)}
                    >
                      Next
                    </Button>
                  </div>
                </div>
              )}
            </CardBody>
          </Card>
        </>
      ) : null}

      {selected && (
        <EntryDialog
          key={selected}
          subdomain={selected}
          onClose={() => setSelected(null)}
          onLifted={() => setOffset(0)}
        />
      )}
    </StandaloneLayout>
  );
}

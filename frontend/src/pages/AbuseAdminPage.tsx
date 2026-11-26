import { useDeferredValue, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Button,
  Card,
  CardBody,
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
import { Ban, Download, Lock, LogOut, ShieldCheck, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { adminAccessError, apiClient } from "@/api/client";
import { AdminAuthOverlay } from "@/components/auth/AdminAuthOverlay";
import { StandaloneLayout } from "@/components/layout/StandaloneLayout";
import { usePageTitle } from "@/hooks/usePageTitle";
import { useAuthStore } from "@/stores/authStore";
import {
  ABUSE_CATEGORIES,
  categoryLabel,
  downloadCsv,
  downloadJson,
  extractSubdomain,
  loadAdminToken,
  saveAdminToken,
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

function SuspendForm({ token }: { token: string }) {
  const queryClient = useQueryClient();
  const [subdomain, setSubdomain] = useState("");
  const [category, setCategory] = useState<AbuseCategory>("malware");
  const [note, setNote] = useState("");
  const label = extractSubdomain(subdomain);

  const suspend = useMutation({
    mutationFn: () =>
      apiClient.suspendSubdomain(token, label ?? "", {
        category,
        note: note.trim() || undefined,
      }),
    onSuccess: (result) => {
      warnIfUnsaved(result.persisted);
      toast.success(
        result.already_suspended
          ? `${result.subdomain} was already suspended; reason updated`
          : `${result.subdomain} is suspended`,
      );
      setSubdomain("");
      setNote("");
      queryClient.invalidateQueries({ queryKey: ["abuse"] });
    },
    onError: () => toast.error("Failed to suspend the subdomain"),
  });

  return (
    <Card>
      <CardBody className="gap-3">
        <h2 className="text-base font-semibold">Suspend a subdomain</h2>
        <div className="flex flex-col gap-3 md:flex-row md:items-end">
          <Input
            label="Subdomain"
            labelPlacement="outside"
            placeholder={`abcd1234.${getDnsDomain()}`}
            value={subdomain}
            onValueChange={setSubdomain}
            autoComplete="off"
            classNames={{ input: "font-mono" }}
            className="md:flex-1"
          />
          <Select
            label="Category"
            labelPlacement="outside"
            selectedKeys={[category]}
            disallowEmptySelection
            onSelectionChange={(keys) => {
              const key = Array.from(keys)[0] as AbuseCategory | undefined;
              if (key) setCategory(key);
            }}
            className="md:w-48"
          >
            {ABUSE_CATEGORIES.map((option) => (
              <SelectItem key={option.key}>{option.label}</SelectItem>
            ))}
          </Select>
          <Input
            label="Note"
            labelPlacement="outside"
            placeholder="Optional"
            value={note}
            onValueChange={setNote}
            maxLength={300}
            className="md:flex-1"
          />
          <Button
            color="danger"
            startContent={<Ban className="h-4 w-4" />}
            isLoading={suspend.isPending}
            isDisabled={!label}
            onPress={() => suspend.mutate()}
          >
            Suspend
          </Button>
        </div>
      </CardBody>
    </Card>
  );
}

interface EntryDialogProps {
  token: string;
  subdomain: string;
  onClose: () => void;
}

/** One suspension: why, what it captured, and the way out. */
function EntryDialog({ token, subdomain, onClose }: EntryDialogProps) {
  const queryClient = useQueryClient();
  const [confirming, setConfirming] = useState<"lift" | "purge" | null>(null);

  const entry = useQuery({
    queryKey: ["abuse", "entry", subdomain],
    queryFn: () => apiClient.getAbuseEntry(token, subdomain),
  });
  const requests = useQuery({
    queryKey: ["abuse", "entry-requests", subdomain],
    queryFn: () => apiClient.getAbuseEntryRequests(token, subdomain),
  });
  const sources = useQuery({
    queryKey: ["abuse", "entry-sources", subdomain],
    queryFn: () => apiClient.getAbuseEntrySources(token, subdomain),
  });

  const lift = useMutation({
    mutationFn: () => apiClient.liftSuspension(token, subdomain),
    onSuccess: ({ persisted }) => {
      warnIfUnsaved(persisted);
      toast.success(`${subdomain} is no longer suspended`);
      queryClient.invalidateQueries({ queryKey: ["abuse"] });
      onClose();
    },
    onError: () => toast.error("Failed to lift the suspension"),
  });
  const purge = useMutation({
    mutationFn: () => apiClient.purgeAbuseEntryData(token, subdomain),
    onSuccess: () => {
      toast.success(`Deleted the data of ${subdomain}`);
      setConfirming(null);
      queryClient.invalidateQueries({ queryKey: ["abuse"] });
    },
    onError: () => toast.error("Failed to delete the data"),
  });

  const data = entry.data;
  const captured = requests.data?.requests ?? [];
  const sourceList = sources.data?.sources ?? [];

  return (
    <Modal isOpen={true} onClose={onClose} size="3xl" scrollBehavior="inside">
      <ModalContent>
        <ModalHeader className="flex flex-wrap items-center gap-2">
          <span className="break-all font-mono">
            {subdomain}.{getDnsDomain()}
          </span>
          {data && (
            <Chip size="sm" variant="flat" color="danger">
              {categoryLabel(data.category)}
            </Chip>
          )}
        </ModalHeader>

        <ModalBody className="gap-6">
          {data && (
            <p className="text-sm text-default-500">
              Suspended {formatDate(data.suspended_at)}{" "}
              {data.source === "admin" ? "by an admin" : "by a report"}.{" "}
              {data.report_count > 0 &&
                `${data.report_count} report${data.report_count === 1 ? "" : "s"}.`}
            </p>
          )}

          {/* Text below comes from reporters or from captured traffic. It is rendered as
              text, never as markup. */}
          {(data?.admin_note || (data && data.reports.length > 0)) && (
            <section className="flex flex-col gap-2">
              <h3 className="text-sm font-semibold">Reports</h3>
              {data?.admin_note && (
                <p className="rounded-lg border border-default-200 p-3 text-sm">
                  <span className="text-default-500">Admin note: </span>
                  {data.admin_note}
                </p>
              )}
              {data?.reports.map((report, index) => (
                <div
                  key={index}
                  className="flex flex-col gap-1 rounded-lg bg-default-100 p-3"
                >
                  <div className="flex flex-wrap items-center gap-2 text-xs text-default-500">
                    <span>{formatDate(report.at)}</span>
                    <span className="font-mono">{report.network}</span>
                    <span>{categoryLabel(report.category)}</span>
                  </div>
                  <p className="whitespace-pre-wrap break-words text-sm">
                    {report.reason ?? (
                      <span className="text-default-400">No reason given</span>
                    )}
                  </p>
                </div>
              ))}
            </section>
          )}

          <section className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-2">
              <h3 className="text-sm font-semibold">
                Sources{" "}
                <span className="font-normal text-default-400">
                  {sources.data?.unique_sources ?? 0}
                </span>
              </h3>
              <Button
                size="sm"
                variant="flat"
                startContent={<Download className="h-4 w-4" />}
                isDisabled={sourceList.length === 0}
                onPress={() =>
                  downloadCsv(
                    `${subdomain}-sources.csv`,
                    [
                      "ip",
                      "country",
                      "requests",
                      "first_seen",
                      "last_seen",
                      "types",
                    ],
                    sourceList.map((source) => [
                      source.ip,
                      source.country,
                      source.requests,
                      new Date(source.first_seen * 1000).toISOString(),
                      new Date(source.last_seen * 1000).toISOString(),
                      source.types.join(" "),
                    ]),
                  )
                }
              >
                CSV
              </Button>
            </div>
            <p className="text-xs text-default-400">
              Every address that contacted it. For a botnet beacon these are its
              victims: hand them to a CERT or abuse.ch.
            </p>
            {sourceList.length > 0 && (
              <Table
                aria-label="Sources"
                isCompact
                removeWrapper
                classNames={{ base: "max-h-56 overflow-auto" }}
              >
                <TableHeader>
                  <TableColumn key="ip">Address</TableColumn>
                  <TableColumn key="requests">Requests</TableColumn>
                  <TableColumn key="last">Last seen</TableColumn>
                </TableHeader>
                <TableBody items={sourceList}>
                  {(source) => (
                    <TableRow key={source.ip}>
                      <TableCell className="font-mono text-xs">
                        <span
                          className={`${getFlagClass(source.country)} mr-1`}
                        />
                        {source.ip}
                      </TableCell>
                      <TableCell className="text-xs">
                        {source.requests}
                      </TableCell>
                      <TableCell className="text-xs">
                        {formatDate(source.last_seen)}
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            )}
          </section>

          <section className="flex flex-col gap-2">
            <div className="flex items-center justify-between gap-2">
              <h3 className="text-sm font-semibold">
                Traffic{" "}
                <span className="font-normal text-default-400">
                  {requests.data?.pagination.total ?? 0}
                </span>
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
                JSON
              </Button>
            </div>
            <p className="text-xs text-default-400">
              Includes what arrived while suspended, which its owner never sees.
              Kept in memory only: export before a redeploy.
            </p>
            {captured.length > 0 && (
              <Table
                aria-label="Traffic"
                isCompact
                removeWrapper
                classNames={{ base: "max-h-64 overflow-auto" }}
              >
                <TableHeader>
                  <TableColumn key="time">Time</TableColumn>
                  <TableColumn key="source">Source</TableColumn>
                  <TableColumn key="request">Request</TableColumn>
                </TableHeader>
                <TableBody items={captured}>
                  {(request) => (
                    <TableRow key={request._id}>
                      <TableCell className="whitespace-nowrap text-xs">
                        {formatDate(request.date)}
                      </TableCell>
                      <TableCell className="font-mono text-xs">
                        {request.ip}
                      </TableCell>
                      <TableCell className="break-all font-mono text-xs">
                        <span className="mr-1 text-default-400">
                          {request.type.toUpperCase()}
                        </span>
                        {summarize(request)}
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            )}
          </section>

          {confirming && (
            <p
              role="alert"
              className="rounded-lg bg-default-100 p-3 text-sm text-foreground"
            >
              {confirming === "lift"
                ? `${subdomain} works again right away, for whoever holds its token. A new report suspends it again.`
                : `This deletes the files, DNS records and captured traffic of ${subdomain}. It cannot be undone; export the sources first if they are to be shared.`}
            </p>
          )}
        </ModalBody>

        <ModalFooter>
          {confirming ? (
            <>
              <Button variant="light" onPress={() => setConfirming(null)}>
                Cancel
              </Button>
              <Button
                color={confirming === "purge" ? "danger" : "primary"}
                isLoading={lift.isPending || purge.isPending}
                onPress={() =>
                  confirming === "lift" ? lift.mutate() : purge.mutate()
                }
              >
                {confirming === "lift" ? "Lift suspension" : "Delete the data"}
              </Button>
            </>
          ) : (
            <>
              <Button
                variant="light"
                startContent={<Trash2 className="h-4 w-4" />}
                onPress={() => setConfirming("purge")}
              >
                Purge data…
              </Button>
              <Button
                color="primary"
                variant="flat"
                startContent={<ShieldCheck className="h-4 w-4" />}
                onPress={() => setConfirming("lift")}
              >
                Lift suspension…
              </Button>
            </>
          )}
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}

/**
 * Suspended subdomains, for whoever holds ABUSE_ADMIN_TOKEN. Outside the dashboard: it
 * creates no session and opens no WebSocket.
 */
export function AbuseAdminPage() {
  usePageTitle("Suspended subdomains");
  const queryClient = useQueryClient();
  const setAuthError = useAuthStore((s) => s.setAuthError);
  const [token, setToken] = useState<string | null>(loadAdminToken);
  const [search, setSearch] = useState("");
  const deferredSearch = useDeferredValue(search);
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);

  const entries = useQuery({
    queryKey: ["abuse", "entries", token, deferredSearch, offset],
    queryFn: () =>
      apiClient.getAbuseEntries(token ?? "", {
        q: deferredSearch.trim() || undefined,
        limit: PAGE_SIZE,
        offset,
      }),
    enabled: token !== null,
    // An access problem is not worth retrying, and a retry would only delay the prompt
    retry: (failureCount, error) =>
      adminAccessError(error) === null && failureCount < 1,
  });

  const access = entries.error ? adminAccessError(entries.error) : null;
  const needsToken = token === null || access === "unauthorized";
  const page = entries.data;

  const handleAuth = async (password: string) => {
    try {
      // Check the token before keeping it
      await apiClient.getAbuseEntries(password, { limit: 1 });
      saveAdminToken(password);
      setToken(password);
      setAuthError(null);
    } catch (error) {
      const problem = adminAccessError(error);
      if (problem === "not_configured") {
        setToken(password);
      } else if (problem === "unauthorized") {
        setAuthError("Invalid admin token");
      } else {
        setAuthError("Could not reach the server. Please try again.");
      }
    }
  };

  const signOut = () => {
    saveAdminToken(null);
    setToken(null);
    queryClient.removeQueries({ queryKey: ["abuse"] });
  };

  return (
    <StandaloneLayout>
      {needsToken && access !== "not_configured" && (
        <AdminAuthOverlay
          title="Abuse admin"
          description="Enter the abuse admin token (ABUSE_ADMIN_TOKEN)."
          onSubmit={handleAuth}
        />
      )}

      <div className="flex items-center justify-between gap-2">
        <div>
          <h1 className="text-xl font-semibold">Suspended subdomains</h1>
          <p className="text-sm text-default-500">
            Reports suspend a subdomain immediately. Suspended subdomains answer{" "}
            <code>410</code>, do not resolve, and keep recording their traffic
            here.
          </p>
        </div>
        {token && access !== "not_configured" && (
          <Button
            size="sm"
            variant="light"
            startContent={<LogOut className="h-4 w-4" />}
            onPress={signOut}
          >
            Sign out
          </Button>
        )}
      </div>

      {access === "not_configured" ? (
        <Card>
          <CardBody className="flex flex-row items-start gap-3">
            <Lock className="mt-0.5 h-5 w-5 shrink-0 text-default-400" />
            <p className="text-sm text-default-500">
              The abuse admin API is off. Set <code>ABUSE_ADMIN_TOKEN</code> on
              the server to manage suspensions here.
            </p>
          </CardBody>
        </Card>
      ) : token ? (
        <>
          <SuspendForm token={token} />

          <Card>
            <CardBody className="gap-3">
              <Input
                placeholder="Search"
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
              <Table
                aria-label="Suspended subdomains"
                selectionMode="none"
                onRowAction={(key) => setSelected(String(key))}
                removeWrapper
              >
                <TableHeader>
                  <TableColumn key="subdomain">Subdomain</TableColumn>
                  <TableColumn key="category">Category</TableColumn>
                  <TableColumn key="by">By</TableColumn>
                  <TableColumn key="reports">Reports</TableColumn>
                  <TableColumn key="since">Suspended</TableColumn>
                </TableHeader>
                <TableBody
                  isLoading={entries.isLoading}
                  loadingContent="Loading…"
                  items={page?.subdomains ?? []}
                  emptyContent={
                    entries.error
                      ? "Could not load the list"
                      : "No suspended subdomains"
                  }
                >
                  {(entry) => (
                    <TableRow key={entry.subdomain} className="cursor-pointer">
                      <TableCell className="font-mono text-sm">
                        {entry.subdomain}
                      </TableCell>
                      <TableCell className="text-sm">
                        {categoryLabel(entry.category)}
                      </TableCell>
                      <TableCell className="text-sm text-default-500">
                        {entry.source === "admin" ? "Admin" : "Report"}
                      </TableCell>
                      <TableCell className="text-sm">
                        {entry.report_count}
                      </TableCell>
                      <TableCell className="text-sm text-default-500">
                        {formatDate(entry.suspended_at)}
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>

              {page && page.pagination.total > PAGE_SIZE && (
                <div className="flex items-center justify-between gap-2">
                  <span className="text-sm text-default-500">
                    {offset + 1}–{offset + page.subdomains.length} of{" "}
                    {page.pagination.total}
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
                      isDisabled={!page.pagination.has_more}
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

      {selected && token && (
        <EntryDialog
          key={selected}
          token={token}
          subdomain={selected}
          onClose={() => setSelected(null)}
        />
      )}
    </StandaloneLayout>
  );
}

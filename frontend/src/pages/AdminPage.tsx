import { useEffect, useState, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import {
  Card,
  Button,
  Input,
  Table,
  Modal,
  useOverlayState,
  Spinner,
  Pagination,
} from "@heroui/react";
import {
  Users,
  Globe,
  FileText,
  RefreshCw,
  Trash2,
  ExternalLink,
  Copy,
  BarChart3,
  X,
  Send,
} from "lucide-react";
import { toast } from "sonner";

interface AdminUser {
  username: string;
  avatar_url: string;
  name: string;
  created_at: string;
  is_admin: boolean;
}

interface AdminSubdomain {
  subdomain: string;
}

interface AdminConfig {
  subdomain_length: number;
  subdomain_alphabet: string;
}

interface LogEntry {
  _id: string;
  type: string;
  method?: string;
  path?: string;
  ip?: string;
  date?: number;
  headers?: Record<string, string>;
  query?: string;
  raw?: string;
  url?: string;
  protocol?: string;
  port?: number;
  name?: string;
  domain?: string;
  query_type?: string;
}

function formatDate(timestamp: number) {
  return new Date(timestamp * 1000).toLocaleString();
}

function formatMethod(method?: string, isDns?: boolean) {
  if (isDns) return "DNS";
  return method || "N/A";
}

function getToken(): string | null {
  try {
    const stored = localStorage.getItem("requestrepo-sessions");
    if (stored) {
      const parsed = JSON.parse(stored);
      const sessions = parsed?.state?.sessions;
      if (sessions?.length > 0) return sessions[0].token;
    }
  } catch {}
  return null;
}

export function AdminPage() {
  const navigate = useNavigate();
  const { isOpen, open, close, setOpen, toggle } = useOverlayState();

  const [users, setUsers] = useState<AdminUser[]>([]);
  const [subdomains, setSubdomains] = useState<AdminSubdomain[]>([]);
  const [config, setConfig] = useState<AdminConfig | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [selectedSubdomain, setSelectedSubdomain] = useState<string | null>(null);
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null);
  const [customSubdomain, setCustomSubdomain] = useState("");
  const [subdomainSearch, setSubdomainSearch] = useState("");
  const [logSearch, setLogSearch] = useState("");
  const [loading, setLoading] = useState({ users: false, subdomains: false, logs: false });
  const [logsPage, setLogsPage] = useState(1);
  const logsPerPage = 10;

  const token = getToken();
  const authHeaders: Record<string, string> = token ? { Authorization: `Bearer ${token}` } : {};

  const fetchUsers = useCallback(async () => {
    setLoading((l) => ({ ...l, users: true }));
    try {
      const res = await fetch("/api/v2/admin/users", { headers: authHeaders });
      if (res.ok) setUsers(await res.json());
    } catch (e) {
      console.error("Failed to fetch users:", e);
    } finally {
      setLoading((l) => ({ ...l, users: false }));
    }
  }, [token]);

  const fetchSubdomains = useCallback(async () => {
    setLoading((l) => ({ ...l, subdomains: true }));
    try {
      const res = await fetch("/api/v2/admin/subdomains", { headers: authHeaders });
      if (res.ok) setSubdomains(await res.json());
    } catch (e) {
      console.error("Failed to fetch subdomains:", e);
    } finally {
      setLoading((l) => ({ ...l, subdomains: false }));
    }
  }, [token]);

  const fetchConfig = useCallback(async () => {
    try {
      const res = await fetch("/api/v2/admin/config", { headers: authHeaders });
      if (res.ok) setConfig(await res.json());
    } catch {}
  }, [token]);

  const fetchLogs = useCallback(
    async (subdomain: string) => {
      setLoading((l) => ({ ...l, logs: true }));
      try {
        const res = await fetch(`/api/v2/admin/logs/${subdomain}`, { headers: authHeaders });
        if (res.ok) setLogs(await res.json());
      } catch (e) {
        console.error("Failed to fetch logs:", e);
      } finally {
        setLoading((l) => ({ ...l, logs: false }));
      }
    },
    [token],
  );

  useEffect(() => {
    if (!token) {
      navigate("/login");
      return;
    }
    fetchUsers();
    fetchSubdomains();
    fetchConfig();
  }, [token, navigate, fetchUsers, fetchSubdomains, fetchConfig]);

  useEffect(() => {
    if (selectedSubdomain) fetchLogs(selectedSubdomain);
  }, [selectedSubdomain, fetchLogs]);

  const handleGetSubdomain = async () => {
    const sub = customSubdomain.trim().toLowerCase();
    if (!sub || !token) return;

    if (config && sub.length !== config.subdomain_length) {
      toast.error(`Subdomain must be exactly ${config.subdomain_length} characters`);
      return;
    }

    try {
      const res = await fetch(`/api/v2/admin/generate_token/${sub}`, {
        method: "POST",
        headers: authHeaders,
      });
      if (res.ok) {
        const data = await res.json();
        window.open(`${window.location.origin}/?share=${data.token}`, "_blank");
        toast.success(`Session opened for ${sub}`);
        fetchSubdomains();
      } else {
        const err = await res.json();
        toast.error(err.error || "Failed to generate token");
      }
    } catch {
      toast.error("Failed to generate token");
    }
  };

  const handleDeleteSubdomain = async (sub: string) => {
    if (!confirm(`Delete subdomain ${sub}? This cannot be undone.`)) return;
    try {
      const res = await fetch(`/api/v2/admin/subdomains/${sub}`, {
        method: "DELETE",
        headers: authHeaders,
      });
      if (res.ok) {
        setSubdomains((s) => s.filter((sd) => sd.subdomain !== sub));
        toast.success(`Subdomain ${sub} deleted`);
        if (selectedSubdomain === sub) setSelectedSubdomain(null);
      }
    } catch {
      toast.error("Failed to delete subdomain");
    }
  };

  const handleDeleteAllSubdomains = async () => {
    if (!confirm("Delete ALL subdomains? This cannot be undone.")) return;
    try {
      const res = await fetch("/api/v2/admin/all-subdomains", {
        method: "DELETE",
        headers: authHeaders,
      });
      if (res.ok) {
        setSubdomains([]);
        setSelectedSubdomain(null);
        setLogs([]);
        toast.success("All subdomains deleted");
      }
    } catch {
      toast.error("Failed to delete all subdomains");
    }
  };

  const handleDeleteAllLogs = async () => {
    if (!selectedSubdomain) return;
    if (!confirm("Delete ALL logs for this subdomain? This cannot be undone.")) return;
    try {
      const res = await fetch(`/api/v2/admin/subdomains/${selectedSubdomain}/logs`, {
        method: "DELETE",
        headers: authHeaders,
      });
      if (res.ok) {
        setLogs([]);
        toast.success("All logs deleted");
      }
    } catch {
      toast.error("Failed to delete logs");
    }
  };

  const handleDeleteLog = async (logId: string) => {
    if (!selectedSubdomain || !token) return;
    try {
      const res = await fetch(
        `/api/v2/admin/subdomains/${selectedSubdomain}/logs/${logId}`,
        { method: "DELETE", headers: authHeaders },
      );
      if (res.ok) {
        setLogs((prev) => prev.filter((l) => l._id !== logId));
        toast.success("Log deleted");
      }
    } catch {
      toast.error("Failed to delete log");
    }
  };

  const handleOpenSession = async (sub: string) => {
    try {
      const res = await fetch(`/api/v2/admin/generate_token/${sub}`, {
        method: "POST",
        headers: authHeaders,
      });
      if (res.ok) {
        const data = await res.json();
        window.open(`${window.location.origin}/?share=${data.token}`, "_blank");
      }
    } catch {
      toast.error("Failed to open session");
    }
  };

  const handleShareSession = async (sub: string) => {
    try {
      const res = await fetch(`/api/v2/admin/generate_token/${sub}`, {
        method: "POST",
        headers: authHeaders,
      });
      if (res.ok) {
        const data = await res.json();
        navigator.clipboard.writeText(`${window.location.origin}/?share=${data.token}`);
        toast.success("Session link copied");
      }
    } catch {
      toast.error("Failed to share session");
    }
  };

  const handleSendToService = async (service: string) => {
    if (!selectedLog || !token) return;
    try {
      const res = await fetch(`/api/v2/notifications/send?token=${token}&service=${service}`, {
        method: "POST",
        headers: { "Content-Type": "application/json", ...authHeaders },
        body: JSON.stringify({
          log: selectedLog,
          message: `Request Log: ${selectedLog.method || "DNS"} ${selectedLog.path || selectedLog.domain || ""}`,
          title: "RequestRepo Log Notification",
        }),
      });
      if (res.ok) toast.success(`Sent to ${service}`);
      else toast.error(`Failed to send to ${service}`);
    } catch {
      toast.error(`Failed to send to ${service}`);
    }
  };

  const filteredSubdomains = subdomains.filter((s) =>
    s.subdomain.toLowerCase().includes(subdomainSearch.toLowerCase()),
  );

  const filteredLogs = logs.filter(
    (log) =>
      !logSearch ||
      log.method?.toLowerCase().includes(logSearch.toLowerCase()) ||
      log.path?.toLowerCase().includes(logSearch.toLowerCase()) ||
      log.ip?.toLowerCase().includes(logSearch.toLowerCase()) ||
      log.domain?.toLowerCase().includes(logSearch.toLowerCase()) ||
      log.headers?.["user-agent"]?.toLowerCase().includes(logSearch.toLowerCase()),
  );

  const pagedLogs = filteredLogs.slice(
    (logsPage - 1) * logsPerPage,
    logsPage * logsPerPage,
  );

  const stats = [
    { label: "Total Users", value: users.length, icon: Users, color: "text-blue-500", bg: "bg-blue-100 dark:bg-blue-900/30" },
    { label: "Total Subdomains", value: subdomains.length, icon: Globe, color: "text-green-500", bg: "bg-green-100 dark:bg-green-900/30" },
    { label: "Total Logs", value: logs.length, icon: FileText, color: "text-orange-500", bg: "bg-orange-100 dark:bg-orange-900/30" },
  ];

  return (
    <div className="max-w-7xl mx-auto flex flex-col gap-6">
      {/* Stats */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        {stats.map((stat) => {
          const Icon = stat.icon;
          return (
            <Card key={stat.label}>
              <Card.Content className="flex flex-row items-center justify-between p-4">
                <div>
                  <div className="text-sm text-default-500 mb-1">{stat.label}</div>
                  <div className="text-3xl font-bold">{stat.value}</div>
                </div>
                <div className={`${stat.bg} rounded-lg p-3`}>
                  <Icon className={`h-6 w-6 ${stat.color}`} />
                </div>
              </Card.Content>
            </Card>
          );
        })}
      </div>

      {/* Two columns: Users + Subdomains */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Users */}
        <Card>
          <Card.Header className="flex flex-col gap-2 px-4 pt-4">
            <h3 className="text-lg font-semibold">Registered Users</h3>
          </Card.Header>
          <Card.Content>
            {loading.users ? (
              <div className="flex justify-center py-8"><Spinner /></div>
            ) : (
              <Table aria-label="Users table">
                <Table.Header>
                  <Table.Column>Username</Table.Column>
                  <Table.Column>Name</Table.Column>
                  <Table.Column>Admin</Table.Column>
                </Table.Header>
                <Table.Body>
                  {users.length === 0 ? (
                    <Table.Row key="empty"><Table.Cell colSpan={3}>No users found</Table.Cell></Table.Row>
                  ) : (
                    users.map((u) => (
                      <Table.Row key={u.username}>
                        <Table.Cell>{u.username}</Table.Cell>
                        <Table.Cell>{u.name}</Table.Cell>
                        <Table.Cell>
                          <span
                            className={`inline-block px-2 py-0.5 rounded text-xs font-medium ${
                              u.is_admin ? "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400" : "bg-default-200 text-default-600"
                            }`}
                          >
                            {u.is_admin ? "Yes" : "No"}
                          </span>
                        </Table.Cell>
                      </Table.Row>
                    ))
                  )}
                </Table.Body>
              </Table>
            )}
          </Card.Content>
        </Card>

        {/* Subdomains */}
        <Card>
          <Card.Header className="flex flex-col gap-2 px-4 pt-4">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-semibold">Subdomains</h3>
              <Button
                size="sm"
                variant="danger"
                onPress={handleDeleteAllSubdomains}
                isDisabled={subdomains.length === 0}
              >
                <Trash2 className="h-3 w-3" />
                Delete All
              </Button>
            </div>
            <div className="flex gap-2 flex-wrap">
              <Input
                placeholder={`${config?.subdomain_length || 8}-char subdomain`}
                value={customSubdomain}
                onChange={(e) => setCustomSubdomain(e.target.value)}
                className="w-40"
              />
              <Button size="sm" variant="primary" onPress={handleGetSubdomain}>
                Get
              </Button>
              <Input
                placeholder="Search..."
                value={subdomainSearch}
                onChange={(e) => setSubdomainSearch(e.target.value)}
                className="flex-1 min-w-[120px]"
              />
              <Button size="sm" variant="secondary" isIconOnly onPress={fetchSubdomains}>
                <RefreshCw className="h-3 w-3" />
              </Button>
            </div>
          </Card.Header>
          <Card.Content>
            {loading.subdomains ? (
              <div className="flex justify-center py-8"><Spinner /></div>
            ) : (
              <Table aria-label="Subdomains table">
                <Table.Header>
                  <Table.Column>Subdomain</Table.Column>
                  <Table.Column>Actions</Table.Column>
                </Table.Header>
                <Table.Body>
                  {filteredSubdomains.length === 0 ? (
                    <Table.Row key="empty"><Table.Cell colSpan={2}>No subdomains found</Table.Cell></Table.Row>
                  ) : (
                    filteredSubdomains.map((s) => (
                      <Table.Row key={s.subdomain}>
                        <Table.Cell className="font-mono text-sm">{s.subdomain}</Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-1">
                            <Button
                              isIconOnly
                              size="sm"
                              variant={selectedSubdomain === s.subdomain ? "primary" : "secondary"}
                              onPress={() => setSelectedSubdomain(s.subdomain)}
                              aria-label="View Logs"
                            >
                              <BarChart3 className="h-4 w-4" />
                            </Button>
                            <Button
                              isIconOnly
                              size="sm"
                              variant="primary"
                              onPress={() => handleOpenSession(s.subdomain)}
                              aria-label="Open Session"
                            >
                              <ExternalLink className="h-4 w-4" />
                            </Button>
                            <Button
                              isIconOnly
                              size="sm"
                              variant="primary"
                              onPress={() => handleShareSession(s.subdomain)}
                              aria-label="Share Session"
                            >
                              <Copy className="h-4 w-4" />
                            </Button>
                            <Button
                              isIconOnly
                              size="sm"
                              variant="danger"
                              onPress={() => handleDeleteSubdomain(s.subdomain)}
                              aria-label="Delete Subdomain"
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </div>
                        </Table.Cell>
                      </Table.Row>
                    ))
                  )}
                </Table.Body>
              </Table>
            )}
          </Card.Content>
        </Card>
      </div>

      {/* Logs Section */}
      {selectedSubdomain && (
        <Card>
          <Card.Header className="flex flex-col gap-2 px-4 pt-4">
            <div className="flex items-center justify-between">
              <h3 className="text-lg font-semibold">Logs - {selectedSubdomain}</h3>
              <div className="flex gap-2">
                <Button
                  size="sm"
                  variant="secondary"
                  onPress={() => fetchLogs(selectedSubdomain)}
                >
                  <RefreshCw className="h-3 w-3" />
                  Refresh
                </Button>
                <Button
                  size="sm"
                  variant="danger"
                  onPress={handleDeleteAllLogs}
                  isDisabled={logs.length === 0}
                >
                  <Trash2 className="h-3 w-3" />
                  Delete All Logs
                </Button>
              </div>
            </div>
            <Input
              placeholder="Search logs by method, path, IP, user-agent..."
              value={logSearch}
              onChange={(e) => setLogSearch(e.target.value)}
              className="w-full"
            />
          </Card.Header>
          <Card.Content>
            {loading.logs ? (
              <div className="flex justify-center py-8"><Spinner /></div>
            ) : (
              <>
                <Table
                  aria-label="Logs table"
                >
                  <Table.Header>
                    <Table.Column>Method</Table.Column>
                    <Table.Column>Path</Table.Column>
                    <Table.Column>IP</Table.Column>
                    <Table.Column>Type</Table.Column>
                    <Table.Column>User Agent</Table.Column>
                    <Table.Column>Date</Table.Column>
                    <Table.Column></Table.Column>
                  </Table.Header>
                  <Table.Body>
                    {pagedLogs.length === 0 ? (
                      <Table.Row key="empty"><Table.Cell colSpan={7}>No logs found</Table.Cell></Table.Row>
                    ) : (
                      pagedLogs.map((log) => {
                        const isDns = log.type === "dns";
                        const methodColor = isDns
                          ? "bg-cyan-500"
                          : log.method === "GET"
                            ? "bg-green-500"
                            : log.method === "POST"
                              ? "bg-amber-500"
                              : log.method === "DELETE"
                                ? "bg-red-500"
                                : log.method === "PUT"
                                  ? "bg-blue-500"
                                  : "bg-gray-500";
                        return (
                          <Table.Row key={log._id} className="cursor-pointer" onClick={() => { setSelectedLog(log); open(); }}>
                            <Table.Cell>
                              <span className={`inline-block px-1.5 py-0.5 rounded-sm text-[10px] font-bold text-white ${methodColor}`}>
                                {formatMethod(log.method, isDns)}
                              </span>
                            </Table.Cell>
                            <Table.Cell className="truncate max-w-[200px] text-xs">
                              {log.path || log.domain || "N/A"}
                            </Table.Cell>
                            <Table.Cell className="text-xs">{log.ip || "N/A"}</Table.Cell>
                            <Table.Cell className="text-xs">
                              {isDns ? log.query_type || "A" : log.headers?.["x-forwarded-scheme"] || "N/A"}
                            </Table.Cell>
                            <Table.Cell className="truncate max-w-[150px] text-xs">
                              {log.headers?.["user-agent"] || "N/A"}
                            </Table.Cell>
                            <Table.Cell className="text-xs whitespace-nowrap">
                              {log.date ? formatDate(log.date) : "N/A"}
                            </Table.Cell>
                            <Table.Cell>
                              <Button
                                isIconOnly
                                size="sm"
                                variant="danger"
                                onPress={() => handleDeleteLog(log._id)}
                                aria-label="Delete log"
                              >
                                <Trash2 className="h-3 w-3" />
                              </Button>
                            </Table.Cell>
                          </Table.Row>
                        );
                      })
                    )}
                  </Table.Body>
                </Table>
                {filteredLogs.length > logsPerPage && (
                  <div className="flex justify-center mt-4">
                    <Pagination>
                      <Pagination.Content>
                        <Pagination.Item>
                          <Pagination.Previous onPress={() => setLogsPage((p) => Math.max(1, p - 1))}>
                            Previous
                          </Pagination.Previous>
                        </Pagination.Item>
                        {Array.from({ length: Math.ceil(filteredLogs.length / logsPerPage) }, (_, i) => (
                          <Pagination.Item key={i + 1}>
                            <Pagination.Link
                              isActive={logsPage === i + 1}
                              onPress={() => setLogsPage(i + 1)}
                            >
                              {i + 1}
                            </Pagination.Link>
                          </Pagination.Item>
                        ))}
                        <Pagination.Item>
                          <Pagination.Next onPress={() => setLogsPage((p) => Math.min(Math.ceil(filteredLogs.length / logsPerPage), p + 1))}>
                            Next
                          </Pagination.Next>
                        </Pagination.Item>
                      </Pagination.Content>
                    </Pagination>
                  </div>
                )}
              </>
            )}
          </Card.Content>
        </Card>
      )}

      {/* Log Detail Modal */}
      <Modal state={{ isOpen, open, close, setOpen, toggle }}>
        <Modal.Backdrop>
          <Modal.Container size="full">
            <Modal.Dialog>
              {selectedLog && (
                <>
                  <Modal.Header className="flex flex-col gap-1">
                    <div className="flex items-center gap-3">
                      <span>Request Details</span>
                      <div className="flex gap-2 ml-auto">
                        <Button size="sm" variant="primary"
                          onPress={() => {
                            navigator.clipboard.writeText(JSON.stringify(selectedLog, null, 2));
                            toast.success("Raw JSON copied");
                          }}
                        >
                          <Copy className="h-3 w-3" />
                          Copy JSON
                        </Button>
                        <Button size="sm" variant="secondary"
                          onPress={() => handleSendToService("discord")}
                          style={{ backgroundColor: "#5865F2", color: "white" }}
                        >
                          <Send className="h-3 w-3" />
                          Discord
                        </Button>
                        <Button size="sm" variant="secondary"
                          onPress={() => handleSendToService("mattermost")}
                          style={{ backgroundColor: "#1E325C", color: "white" }}
                        >
                          <Send className="h-3 w-3" />
                          Mattermost
                        </Button>
                        <Button size="sm" variant="secondary"
                          onPress={() => handleSendToService("telegram")}
                          style={{ backgroundColor: "#229ED9", color: "white" }}
                        >
                          <Send className="h-3 w-3" />
                          Telegram
                        </Button>
                      </div>
                    </div>
                  </Modal.Header>
                  <Modal.Body>
                    <div className="space-y-6">
                      {/* Request Details Table */}
                      <div>
                        <h4 className="font-semibold mb-2 text-sm">Request Details</h4>
                        <table className="w-full text-sm req-table">
                          <tbody>
                            {[
                              ["ID", selectedLog._id || "N/A"],
                              ["Method", selectedLog.method ? (
                                <span className={`inline-block px-1.5 py-0.5 rounded text-xs font-bold text-white ${
                                  selectedLog.type === "dns" ? "bg-cyan-500" :
                                  selectedLog.method === "GET" ? "bg-green-500" :
                                  selectedLog.method === "POST" ? "bg-amber-500" :
                                  selectedLog.method === "DELETE" ? "bg-red-500" : "bg-gray-500"
                                }`}>{selectedLog.method}</span>
                              ) : "DNS"],
                              ["Path", selectedLog.path || selectedLog.domain || "N/A"],
                              ["Date", selectedLog.date ? formatDate(selectedLog.date) : "N/A"],
                              ["IP Address", selectedLog.ip || "N/A"],
                              ["Port", String(selectedLog.port || "N/A")],
                              ["URL", selectedLog.url || "N/A"],
                              ["Protocol", selectedLog.protocol || "N/A"],
                            ].map(([label, value]) => (
                              <tr key={String(label)} className="border-b border-default-200">
                                <td className="py-1.5 font-semibold pr-4 w-32 text-default-500">{String(label)}:</td>
                                <td className="py-1.5">{typeof value === "string" ? value : value}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>

                      {/* Headers Section */}
                      {selectedLog.headers && Object.keys(selectedLog.headers).length > 0 && (
                        <div>
                          <h4 className="font-semibold mb-2 text-sm">Headers</h4>
                          <table className="w-full text-sm req-table">
                            <tbody>
                              {Object.entries(selectedLog.headers).map(([k, v]) => (
                                <tr key={k} className="border-b border-default-200">
                                  <td className="py-1 font-semibold pr-4 w-32 text-default-500 break-all">{k}:</td>
                                  <td className="py-1 break-all">{v}</td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                      )}

                      {/* Query Parameters Section */}
                      {selectedLog.query && selectedLog.query.length > 1 && (
                        <div>
                          <h4 className="font-semibold mb-2 text-sm">Query Parameters</h4>
                          <table className="w-full text-sm req-table">
                            <tbody>
                              {selectedLog.query
                                .substring(1)
                                .split("&")
                                .map((param, index) => {
                                  const [key, value] = param.split("=");
                                  return (
                                    <tr key={index} className="border-b border-default-200">
                                      <td className="py-1 font-semibold pr-4 w-32 text-default-500">{key}:</td>
                                      <td className="py-1">{value || ""}</td>
                                    </tr>
                                  );
                                })}
                            </tbody>
                          </table>
                        </div>
                      )}

                      {/* Form Data / Body Section */}
                      {selectedLog.raw && (
                        <div>
                          <h4 className="font-semibold mb-2 text-sm">Form Data / Body</h4>
                          <pre className="bg-default-100 rounded-sm p-3 text-xs overflow-auto max-h-60 whitespace-pre-wrap break-all">
                            {(() => {
                              try { return atob(selectedLog.raw); } catch { return selectedLog.raw; }
                            })()}
                          </pre>
                        </div>
                      )}

                      {/* Raw Request Section */}
                      <div>
                        <h4 className="font-semibold mb-2 text-sm">Raw Request</h4>
                        <pre className="bg-default-100 rounded-sm p-3 text-xs overflow-auto max-h-80 whitespace-pre-wrap break-all">
                          {(() => {
                            let raw = "";
                            raw += `${selectedLog.method || "GET"} ${selectedLog.path || "/"}${selectedLog.query || ""} ${selectedLog.protocol || "HTTP/1.1"}\n`;
                            if (selectedLog.headers) {
                              Object.entries(selectedLog.headers).forEach(([k, v]) => {
                                raw += `${k}: ${v}\n`;
                              });
                            }
                            if (selectedLog.raw) {
                              raw += "\n";
                              try { raw += atob(selectedLog.raw); } catch { raw += selectedLog.raw; }
                            }
                            return raw;
                          })()}
                        </pre>
                      </div>
                    </div>
                  </Modal.Body>
                  <Modal.Footer>
                    <Button
                      variant="danger"
                      onPress={() => {
                        if (selectedLog._id) handleDeleteLog(selectedLog._id);
                        close();
                      }}
                    >
                      <Trash2 className="h-4 w-4" />
                      Delete
                    </Button>
                    <Button variant="secondary" onPress={close}>
                      <X className="h-4 w-4" />
                      Close
                    </Button>
                  </Modal.Footer>
                </>
              )}
            </Modal.Dialog>
          </Modal.Container>
        </Modal.Backdrop>
      </Modal>
    </div>
  );
}

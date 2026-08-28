import axios, { AxiosError } from "axios";
import type {
  SessionCreateResponse,
  DnsRecord,
  FileTree,
  NotificationSettings,
  PaginatedResponse,
  Request,
} from "@/types";

const api = axios.create({
  baseURL: "/api/v2",
  headers: {
    "Content-Type": "application/json",
  },
  timeout: 10000, // 10 second timeout
  withCredentials: true, // Send cookies with requests (for admin_token)
});

const authHeaders = (token: string) => ({
  Authorization: `Bearer ${token}`,
});

// Helper to check if we're in offline mode
function isNetworkError(error: unknown): boolean {
  if (error instanceof AxiosError) {
    return (
      error.code === "ERR_NETWORK" ||
      error.code === "ECONNABORTED" ||
      error.message === "Network Error" ||
      !error.response
    );
  }
  return false;
}

// Helper to check if error is "admin token required"
export function isAdminRequiredError(error: unknown): boolean {
  if (error instanceof AxiosError && error.response?.status === 403) {
    const data = error.response.data as { code?: string } | undefined;
    return data?.code === "admin_required";
  }
  return false;
}

// Session API
export async function createSession(
  adminToken?: string,
): Promise<SessionCreateResponse> {
  const payload = adminToken ? { admin_token: adminToken } : {};
  const { data } = await api.post<SessionCreateResponse>("/sessions", payload);
  return data;
}

// DNS API
export async function getDnsRecords(token: string): Promise<DnsRecord[]> {
  try {
    const { data } = await api.get<{ records: DnsRecord[] }>("/dns", {
      headers: authHeaders(token),
    });
    return data.records || [];
  } catch (error) {
    if (isNetworkError(error)) {
      console.log("DNS API offline - returning empty records");
      return [];
    }
    throw error;
  }
}

export async function updateDnsRecords(
  token: string,
  records: DnsRecord[],
): Promise<void> {
  await api.put("/dns", { records }, { headers: authHeaders(token) });
}

// Files API
export async function getFiles(token: string): Promise<FileTree> {
  try {
    const { data } = await api.get<FileTree>("/files", {
      headers: authHeaders(token),
    });
    return data;
  } catch (error) {
    if (isNetworkError(error)) {
      console.log("Files API offline - returning default file");
      return {
        "index.html": {
          raw: btoa("<h1>Hello from RequestRepo!</h1>"),
          status_code: 200,
          headers: [{ header: "Content-Type", value: "text/html" }],
        },
      };
    }
    throw error;
  }
}

export async function updateFiles(
  token: string,
  files: FileTree,
): Promise<void> {
  await api.put("/files", files, { headers: authHeaders(token) });
}

// Requests API
export async function getRequests(
  token: string,
  limit = 100,
  offset = 0,
): Promise<PaginatedResponse<Request>> {
  try {
    const { data } = await api.get<PaginatedResponse<Request>>("/requests", {
      headers: authHeaders(token),
      params: { limit, offset },
    });
    return data;
  } catch (error) {
    if (isNetworkError(error)) {
      console.log("Requests API offline - returning empty list");
      return {
        requests: [],
        pagination: { total: 0, limit, offset, has_more: false },
      };
    }
    throw error;
  }
}

export async function getRequest(
  token: string,
  requestId: string,
): Promise<Request> {
  const { data } = await api.get<Request>(`/requests/${requestId}`, {
    headers: authHeaders(token),
  });
  return data;
}

export async function deleteRequest(
  token: string,
  requestId: string,
): Promise<void> {
  await api.delete(`/requests/${requestId}`, {
    headers: authHeaders(token),
  });
}

export async function deleteAllRequests(token: string): Promise<void> {
  await api.delete("/requests", {
    headers: authHeaders(token),
  });
}

// Create a share token for a request (requires auth)
export async function createShareToken(
  token: string,
  requestId: string,
): Promise<string> {
  const { data } = await api.post<{ share_token: string }>(
    `/requests/${requestId}/share`,
    {},
    { headers: authHeaders(token) },
  );
  return data.share_token;
}

// Get a shared request by share token (public endpoint, no auth required)
export async function getSharedRequest(
  shareToken: string,
): Promise<Request | null> {
  try {
    const { data } = await api.get<Request>(`/requests/shared/${shareToken}`);
    return data;
  } catch {
    return null;
  }
}

// Notification Settings API
export async function getNotificationSettings(
  token: string,
): Promise<NotificationSettings> {
  try {
    const { data } = await api.get<NotificationSettings>(
      "/notifications/settings",
      { headers: authHeaders(token) },
    );
    return data;
  } catch (error) {
    if (isNetworkError(error)) {
      return {
        discord_webhook_url: "",
        mattermost_webhook_url: "",
        telegram_bot_token: "",
        telegram_chat_id: "",
      };
    }
    throw error;
  }
}

export async function updateNotificationSettings(
  token: string,
  settings: NotificationSettings,
): Promise<void> {
  await api.put("/notifications/settings", settings, { headers: authHeaders(token) });
}

export async function sendTestNotification(
  token: string,
  service: string,
): Promise<void> {
  await api.post(
    "/notifications/test",
    {
      message: "This is a test notification from RequestRepo",
      title: "RequestRepo Test Notification",
    },
    { headers: authHeaders(token), params: { service } },
  );
}

export async function sendRequestNotification(
  token: string,
  service: string,
  log: Record<string, unknown>,
  message?: string,
  title?: string,
): Promise<void> {
  await api.post(
    "/notifications/send",
    { log, message, title },
    { headers: authHeaders(token), params: { service } },
  );
}

export const apiClient = {
  createSession,
  getDnsRecords,
  updateDnsRecords,
  getFiles,
  updateFiles,
  getRequests,
  getRequest,
  createShareToken,
  getSharedRequest,
  deleteRequest,
  deleteAllRequests,
  getNotificationSettings,
  updateNotificationSettings,
  sendTestNotification,
  sendRequestNotification,
};

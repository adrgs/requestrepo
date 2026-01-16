import axios, { AxiosError } from "axios";
import type {
  SessionCreateResponse,
  DnsRecord,
  FileTree,
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
      params: { token },
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
  await api.put("/dns", { records }, { params: { token } });
}

// Files API
export async function getFiles(token: string): Promise<FileTree> {
  try {
    const { data } = await api.get<FileTree>("/files", {
      params: { token },
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
  await api.put("/files", files, { params: { token } });
}

// Requests API
export async function getRequests(
  token: string,
  limit = 100,
  offset = 0,
): Promise<PaginatedResponse<Request>> {
  try {
    const { data } = await api.get<PaginatedResponse<Request>>("/requests", {
      params: { token, limit, offset },
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
    params: { token },
  });
  return data;
}

export async function deleteRequest(
  token: string,
  requestId: string,
): Promise<void> {
  await api.delete(`/requests/${requestId}`, {
    params: { token },
  });
}

export async function deleteAllRequests(token: string): Promise<void> {
  await api.delete("/requests", {
    params: { token },
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
    { params: { token } },
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
};

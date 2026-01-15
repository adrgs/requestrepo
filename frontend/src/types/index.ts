// Session types
export interface Session {
  subdomain: string;
  token: string;
  createdAt: string;
}

// Request types (discriminated union)
export interface BaseRequest {
  _id: string;
  type: "http" | "dns" | "smtp";
  date: number;
  ip: string;
  port?: number;
  country?: string;
  raw: string;
  uid: string;
}

export interface HttpRequest extends BaseRequest {
  type: "http";
  method: string;
  path: string;
  query: string | null;
  fragment: string | null;
  protocol: string;
  url: string;
  headers: Record<string, string>;
}

export interface DnsRequest extends BaseRequest {
  type: "dns";
  domain: string; // The queried domain name
  query_type: string; // DNS record type (A, AAAA, TXT, etc)
  reply?: string; // Response (optional, added for display)
}

export interface SmtpRequest extends BaseRequest {
  type: "smtp";
  command: string; // SMTP command (DATA, MAIL, RCPT, etc.)
  data: string | null; // Email body for DATA commands
}

export type Request = HttpRequest | DnsRequest | SmtpRequest;

// Type guard functions
export function isHttpRequest(request: Request): request is HttpRequest {
  return request.type === "http";
}

export function isDnsRequest(request: Request): request is DnsRequest {
  return request.type === "dns";
}

export function isSmtpRequest(request: Request): request is SmtpRequest {
  return request.type === "smtp";
}

// DNS Records
export type DnsRecordType = "A" | "AAAA" | "CNAME" | "TXT";

export interface DnsRecord {
  domain: string;
  type: DnsRecordType;
  value: string;
}

// Files
export interface ResponseHeader {
  header: string;
  value: string;
}

export interface FileEntry {
  raw: string;
  headers: ResponseHeader[];
  status_code: number;
}

export type FileTree = Record<string, FileEntry>;

// API Response types
export interface SessionCreateResponse {
  subdomain: string;
  token: string;
}

export interface PaginatedResponse<T> {
  requests: T[];
  pagination: {
    total: number;
    limit: number;
    offset: number;
    has_more: boolean;
  };
}

// WebSocket types
export type WebSocketClientMessage =
  | { cmd: "connect"; token: string }
  | { cmd: "ping" }
  | { cmd: "disconnect"; subdomain?: string };

export type WebSocketServerMessage =
  | { cmd: "connected"; subdomain: string }
  | { cmd: "pong" }
  | { cmd: "request"; subdomain: string; data: Request }
  | { cmd: "requests"; subdomain: string; data: Request[] }
  | { cmd: "deleted"; subdomain: string; data: { _id: string } }
  | { cmd: "cleared"; subdomain: string }
  | { cmd: "error"; code: string; message: string };

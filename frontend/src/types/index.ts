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

export interface SmtpAttachment {
  filename: string;
  content_type: string;
  size: number;
  content: string; // base64
}

export interface SmtpRequest extends BaseRequest {
  type: "smtp";
  command: string; // SMTP command (DATA, MAIL, RCPT, etc.)
  data: string | null; // Email body for DATA commands
  // Parsed email headers (from backend using mail-parser)
  subject: string | null;
  from: string | null;
  to: string | null;
  cc: string | null;
  bcc: string | null;
  text_body?: string;
  html_body?: string;
  attachments?: SmtpAttachment[];
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

// Abuse reporting: a report blocks the subdomain
export type AbuseCategory =
  | "malware"
  | "phishing"
  | "csam"
  | "spam"
  | "copyright"
  | "other";

export interface AbuseReport {
  at: number;
  category: AbuseCategory;
  // Reporter's network, such as "203.0.113.0/24"
  network: string;
  description: string;
  evidence_url?: string;
  contact?: string;
}

export interface AbuseEntrySummary {
  subdomain: string;
  category: AbuseCategory;
  source: "report" | "admin";
  created_at: number;
  report_count: number;
}

export type AbuseEntry = AbuseEntrySummary & {
  reports: AbuseReport[];
  admin_note?: string;
};

export interface CreateAbuseReportPayload {
  subdomain: string;
  category: AbuseCategory;
  description: string;
  evidence_url?: string;
  contact?: string;
}

export interface AbuseReportResponse {
  status: "blocked" | "already_blocked";
  subdomain: string;
  // False when the block took effect but could not be saved to disk
  persisted: boolean;
}

export interface AbuseEntriesResponse {
  entries: AbuseEntrySummary[];
  total: number;
  limit: number;
  offset: number;
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
  // Sent to a live dashboard when its session gets blocked
  | { cmd: "blacklisted"; subdomain: string; category?: AbuseCategory }
  | {
      cmd: "error";
      code: string;
      message: string;
      // Set when the code is "subdomain_blacklisted"
      subdomain?: string;
      category?: AbuseCategory;
    };

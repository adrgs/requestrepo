export interface HttpRequest {
  _id: string;
  type: "http";
  new?: boolean;
  method: string;
  path: string;
  query_string: string;
  headers: Record<string, string>;
  body: string;
  ip: string;
  timestamp: string;
  country?: string;
}

export interface DnsRequest {
  _id: string;
  type: "dns";
  new?: boolean;
  query: string;
  record_type: string;
  timestamp: string;
}

export type Request = HttpRequest | DnsRequest;

export interface AppSession {
  url: string;
  domain: string;
  subdomain: string;
  httpRequests: HttpRequest[];
  dnsRequests: DnsRequest[];
  timestamp: string | null;
  requests: Record<string, Request>;
  visited: Record<string, boolean>;
  selectedRequest: string | null;
  token: string;
  dnsRecords?: DnsRecord[];
}

export interface DnsRecord {
  name: string;
  type: string;
  content: string;
  ttl: number;
  id?: string;
}

export interface AppState {
  layoutMode: string;
  layoutColorMode: string;
  staticMenuInactive: boolean;
  overlayMenuActive: boolean;
  mobileMenuActive: boolean;
  sessions: Record<string, AppSession>;
  activeSession: string;
  searchValue: string;
  response: {
    raw: string;
    headers: Array<{ key: string; value: string }>;
    status_code: number;
    fetched: boolean;
  };
  dnsRecords: DnsRecord[];
  dnsFetched: boolean;
}

export interface ToastFunctions {
  info: (message: string, options?: Record<string, unknown>) => void;
  success: (message: string, options?: Record<string, unknown>) => void;
  error: (message: string, options?: Record<string, unknown>) => void;
  warning: (message: string, options?: Record<string, unknown>) => void;
}

export interface SessionData {
  subdomain: string;
  token: string;
  createdAt: string;
  unseenRequests: number;
}

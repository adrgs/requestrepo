export interface HttpRequest {
  _id: string;
  type: "http";
  new?: boolean;
  method: string;
  path: string;
  query_string: string;
  query?: string;
  fragment?: string;
  headers: Record<string, string>;
  body: string;
  ip: string;
  timestamp: string;
  date: number;
  country?: string;
}

export interface DnsRequest {
  _id: string;
  type: "dns";
  new?: boolean;
  query: string;
  record_type: string;
  timestamp: string;
  date: number;
  name?: string;
  ip?: string;
  country?: string;
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

export interface ToastOptions {
  position?:
    | "top-right"
    | "top-center"
    | "top-left"
    | "bottom-right"
    | "bottom-center"
    | "bottom-left";
  autoClose?: number | false;
  hideProgressBar?: boolean;
  closeOnClick?: boolean;
  pauseOnHover?: boolean;
  draggable?: boolean;
  progress?: number;
  theme?: "light" | "dark" | "colored";
  className?: string;
  style?: React.CSSProperties;
  dark?: boolean; // Support for dark mode toggle
}

export interface ToastFunctions {
  info: (message: string, options?: ToastOptions) => number;
  success: (message: string, options?: ToastOptions) => number;
  error: (message: string, options?: ToastOptions) => number;
  warning: (message: string, options?: ToastOptions) => number;
  [key: string]: (message: string, options?: ToastOptions) => number; // Type-safe index signature
}

export interface SessionData {
  subdomain: string;
  token: string;
  createdAt: string;
  unseenRequests: number;
}

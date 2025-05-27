interface ImportMetaEnv {
  readonly VITE_DOMAIN?: string;
  readonly DEV: boolean;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare module "react-toastify" {
  import React from "react";

  export interface ToastOptions {
    position?: string;
    autoClose?: number;
    hideProgressBar?: boolean;
    closeOnClick?: boolean;
    pauseOnHover?: boolean;
    draggable?: boolean;
    dark?: boolean;
    theme?: "light" | "dark";
  }

  export interface ToastResponse {
    error?: string;
    msg?: string;
  }

  export const toast: {
    info: (message: string, options?: ToastOptions) => void;
    success: (message: string, options?: ToastOptions) => void;
    error: (message: string, options?: ToastOptions) => void;
    warning: (message: string, options?: ToastOptions) => void;
  };

  export const ToastContainer: React.FC<ToastOptions>;
}

interface UserSession {
  subdomain: string;
  domain: string;
  url?: string;
  httpRequests?: Array<{
    _id: string;
    date: string;
    method: string;
    path: string;
  }>;
  dnsRequests?: Array<{
    _id: string;
    date: string;
    query: string;
    record_type: string;
  }>;
  timestamp?: string | null;
  requests?: Record<string, HttpRequest | DnsRequest>;
  visited?: Record<string, boolean>;
  selectedRequest?: string | null;
  token: string;
  dnsRecords?: Array<DnsRecord>;
}

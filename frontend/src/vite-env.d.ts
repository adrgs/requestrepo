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
    [key: string]: any;
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

  export const ToastContainer: React.FC<any>;
}

interface UserSession {
  subdomain: string;
  domain: string;
  url?: string;
  httpRequests?: any[];
  dnsRequests?: any[];
  timestamp?: string | null;
  requests?: Record<string, any>;
  visited?: Record<string, boolean>;
  selectedRequest?: string | null;
  token: string;
  dnsRecords?: any[];
}

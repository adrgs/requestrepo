import { create } from "zustand";
import type { Request } from "@/types";

// Stable empty array reference to avoid infinite loops in React
const EMPTY_REQUESTS: Request[] = [];

interface RequestState {
  // Requests per subdomain (managed by WebSocket, NOT persisted)
  requests: Record<string, Request[]>;

  // Actions
  setRequests: (subdomain: string, requests: Request[]) => void;
  addRequest: (subdomain: string, request: Request) => void;
  removeRequest: (subdomain: string, requestId: string) => void;
  clearRequests: (subdomain: string) => void;
  getRequests: (subdomain: string) => Request[];
}

// NO persist middleware - requests come from WebSocket only
export const useRequestStore = create<RequestState>()((set, get) => ({
  requests: {},

  setRequests: (subdomain, requests) =>
    set((state) => ({
      requests: {
        ...state.requests,
        [subdomain]: requests,
      },
    })),

  addRequest: (subdomain, request) =>
    set((state) => {
      const existing = state.requests[subdomain] ?? [];
      // Avoid duplicates
      if (existing.some((r) => r._id === request._id)) {
        return state;
      }
      return {
        requests: {
          ...state.requests,
          [subdomain]: [...existing, request],
        },
      };
    }),

  removeRequest: (subdomain, requestId) =>
    set((state) => ({
      requests: {
        ...state.requests,
        [subdomain]: (state.requests[subdomain] ?? []).filter(
          (r) => r._id !== requestId,
        ),
      },
    })),

  clearRequests: (subdomain) =>
    set((state) => ({
      requests: {
        ...state.requests,
        [subdomain]: [],
      },
    })),

  getRequests: (subdomain) => {
    return get().requests[subdomain] ?? EMPTY_REQUESTS;
  },
}));

// ============================================
// Cross-tab sync via pulse notifications
// ============================================

// Broadcast a single request deletion to other tabs
export function broadcastDeleteRequest(subdomain: string, requestId: string) {
  const key = `requestrepo-deleted-${subdomain}`;
  localStorage.setItem(key, requestId);
  setTimeout(() => localStorage.removeItem(key), 100);
}

// Broadcast a clear-all to other tabs
export function broadcastClearRequests(subdomain: string) {
  const key = `requestrepo-cleared-${subdomain}`;
  localStorage.setItem(key, Date.now().toString());
  setTimeout(() => localStorage.removeItem(key), 100);
}

// Listen for pulse notifications from other tabs
if (typeof window !== "undefined") {
  // Clean up old persisted requests key (no longer used)
  localStorage.removeItem("requestrepo-requests");

  window.addEventListener("storage", (e) => {
    if (!e.key || !e.newValue) return;

    if (e.key.startsWith("requestrepo-deleted-")) {
      const subdomain = e.key.replace("requestrepo-deleted-", "");
      const requestId = e.newValue;
      useRequestStore.getState().removeRequest(subdomain, requestId);
    } else if (e.key.startsWith("requestrepo-cleared-")) {
      const subdomain = e.key.replace("requestrepo-cleared-", "");
      useRequestStore.getState().clearRequests(subdomain);
    }
  });
}

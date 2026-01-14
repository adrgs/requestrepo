import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { Request } from "@/types";

interface UiState {
  // Request filters
  httpFilter: boolean;
  dnsFilter: boolean;
  searchQuery: string;

  // Request selection
  selectedRequestId: string | null;

  // Shared request (for viewing a single shared request without session)
  sharedRequest: Request | null;

  // Visited tracking per subdomain
  visitedRequests: Record<string, Record<string, boolean>>;

  // Actions
  setFilters: (http: boolean, dns: boolean) => void;
  setSearchQuery: (query: string) => void;
  selectRequest: (requestId: string | null) => void;
  setSharedRequest: (request: Request | null) => void;
  markRequestVisited: (subdomain: string, requestId: string) => void;
  markAllAsRead: (subdomain: string, requestIds: string[]) => void;
  isRequestVisited: (subdomain: string, requestId: string) => boolean;
  clearVisited: (subdomain: string) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    (set, get) => ({
      httpFilter: true,
      dnsFilter: true,
      searchQuery: "",
      selectedRequestId: null,
      sharedRequest: null,
      visitedRequests: {},

      setFilters: (http, dns) => set({ httpFilter: http, dnsFilter: dns }),
      setSearchQuery: (query) => set({ searchQuery: query }),
      selectRequest: (requestId) => set({ selectedRequestId: requestId, sharedRequest: null }),
      setSharedRequest: (request) => set({ sharedRequest: request }),

      markRequestVisited: (subdomain, requestId) =>
        set((state) => ({
          visitedRequests: {
            ...state.visitedRequests,
            [subdomain]: {
              ...state.visitedRequests[subdomain],
              [requestId]: true,
            },
          },
        })),

      markAllAsRead: (subdomain, requestIds) =>
        set((state) => ({
          visitedRequests: {
            ...state.visitedRequests,
            [subdomain]: {
              ...state.visitedRequests[subdomain],
              ...Object.fromEntries(requestIds.map((id) => [id, true])),
            },
          },
        })),

      isRequestVisited: (subdomain, requestId) => {
        const { visitedRequests } = get();
        return visitedRequests[subdomain]?.[requestId] ?? false;
      },

      clearVisited: (subdomain) =>
        set((state) => ({
          visitedRequests: {
            ...state.visitedRequests,
            [subdomain]: {},
          },
        })),
    }),
    {
      name: "requestrepo-ui",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({
        visitedRequests: state.visitedRequests,
        selectedRequestId: state.selectedRequestId,
      }),
    }
  )
);

// Cross-tab sync: listen for storage events from other tabs
if (typeof window !== "undefined") {
  window.addEventListener("storage", (e) => {
    if (e.key === "requestrepo-ui" && e.newValue) {
      try {
        const parsed = JSON.parse(e.newValue);
        if (parsed.state) {
          useUiStore.setState({
            visitedRequests: parsed.state.visitedRequests ?? {},
            selectedRequestId: parsed.state.selectedRequestId ?? null,
          });
        }
      } catch {
        // Ignore parse errors
      }
    }
  });
}

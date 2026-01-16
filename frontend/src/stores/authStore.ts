import { create } from "zustand";

interface AuthState {
  // Whether we're currently showing the auth overlay
  showAuthOverlay: boolean;

  // Error message if auth failed
  authError: string | null;

  // Backend connection status
  backendOffline: boolean;

  // Actions
  setShowAuthOverlay: (show: boolean) => void;
  setAuthError: (error: string | null) => void;
  setBackendOffline: (offline: boolean) => void;
}

export const useAuthStore = create<AuthState>()((set) => ({
  showAuthOverlay: false,
  authError: null,
  backendOffline: false,

  setShowAuthOverlay: (show) => set({ showAuthOverlay: show, authError: null }),
  setAuthError: (error) => set({ authError: error }),
  setBackendOffline: (offline) => set({ backendOffline: offline }),
}));

import { create } from "zustand";

interface BlockedState {
  // Subdomains the server has answered with `subdomain_suspended`
  blocked: Record<string, true>;
  markBlocked: (subdomain: string) => void;
}

// Not persisted on purpose: this is the server's answer, and it is asked for again on
// every load.
export const useBlockedStore = create<BlockedState>()((set) => ({
  blocked: {},

  markBlocked: (subdomain) =>
    set((state) =>
      state.blocked[subdomain]
        ? state
        : { blocked: { ...state.blocked, [subdomain]: true } },
    ),
}));

import { create } from "zustand";
import type { AbuseCategory } from "@/types";

interface BlockedState {
  // Subdomains the server has answered with `subdomain_blacklisted`, and why
  blocked: Record<string, { category?: AbuseCategory }>;
  markBlocked: (subdomain: string, category?: AbuseCategory) => void;
}

// Not persisted on purpose: this is the server's answer, and it is asked for again on
// every load. A block that has been lifted is forgotten by reloading.
export const useBlockedStore = create<BlockedState>()((set) => ({
  blocked: {},

  markBlocked: (subdomain, category) =>
    set((state) => {
      const known = state.blocked[subdomain];
      // A later answer without a category must not erase one already known
      const merged = category ?? known?.category;
      if (known && merged === known.category) return state;
      return {
        blocked: { ...state.blocked, [subdomain]: { category: merged } },
      };
    }),
}));

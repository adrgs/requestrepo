import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type { Session } from "@/types";

interface SessionState {
  sessions: Session[];
  activeSubdomain: string | null;

  // Actions
  addSession: (session: Session) => void;
  removeSession: (subdomain: string) => void;
  replaceSession: (oldSubdomain: string, newSession: Session) => void;
  setActiveSession: (subdomain: string) => void;
  getActiveSession: () => Session | undefined;
}

export const useSessionStore = create<SessionState>()(
  persist(
    (set, get) => ({
      sessions: [],
      activeSubdomain: null,

      addSession: (session) =>
        set((state) => ({
          sessions: [...state.sessions, session],
          activeSubdomain: session.subdomain,
        })),

      removeSession: (subdomain) =>
        set((state) => {
          const filtered = state.sessions.filter(
            (s) => s.subdomain !== subdomain
          );
          const newActive =
            state.activeSubdomain === subdomain
              ? (filtered[0]?.subdomain ?? null)
              : state.activeSubdomain;
          return { sessions: filtered, activeSubdomain: newActive };
        }),

      replaceSession: (oldSubdomain, newSession) =>
        set((state) => {
          const sessions = state.sessions.map((s) =>
            s.subdomain === oldSubdomain ? newSession : s
          );
          const newActive =
            state.activeSubdomain === oldSubdomain
              ? newSession.subdomain
              : state.activeSubdomain;
          return { sessions, activeSubdomain: newActive };
        }),

      setActiveSession: (subdomain) => set({ activeSubdomain: subdomain }),

      getActiveSession: () => {
        const { sessions, activeSubdomain } = get();
        return sessions.find((s) => s.subdomain === activeSubdomain);
      },
    }),
    {
      name: "requestrepo-sessions",
      storage: createJSONStorage(() => localStorage),
    }
  )
);

import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";

type Theme = "light" | "dark" | "system";

interface ThemeState {
  theme: Theme;
  resolvedTheme: "light" | "dark";
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
  initTheme: () => void;
}

function getResolvedTheme(theme: Theme): "light" | "dark" {
  if (theme === "system") {
    return window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  }
  return theme;
}

function applyTheme(resolvedTheme: "light" | "dark") {
  const root = window.document.documentElement;
  root.classList.toggle("dark", resolvedTheme === "dark");
}

export const useThemeStore = create<ThemeState>()(
  persist(
    (set, get) => ({
      theme: "system",
      resolvedTheme: "light",

      setTheme: (theme) => {
        const resolvedTheme = getResolvedTheme(theme);
        applyTheme(resolvedTheme);
        set({ theme, resolvedTheme });
      },

      toggleTheme: () => {
        const { resolvedTheme } = get();
        const newTheme = resolvedTheme === "dark" ? "light" : "dark";
        applyTheme(newTheme);
        set({ theme: newTheme, resolvedTheme: newTheme });
      },

      initTheme: () => {
        const { theme } = get();
        const resolvedTheme = getResolvedTheme(theme);
        applyTheme(resolvedTheme);
        set({ resolvedTheme });

        // Listen for system theme changes
        if (theme === "system") {
          const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
          const handler = (e: MediaQueryListEvent) => {
            const newResolved = e.matches ? "dark" : "light";
            applyTheme(newResolved);
            set({ resolvedTheme: newResolved });
          };
          mediaQuery.addEventListener("change", handler);
        }
      },
    }),
    {
      name: "requestrepo-theme",
      storage: createJSONStorage(() => localStorage),
      partialize: (state) => ({ theme: state.theme }),
      onRehydrateStorage: () => (state) => {
        // Apply theme after rehydration
        if (state) {
          const resolvedTheme = getResolvedTheme(state.theme);
          applyTheme(resolvedTheme);
          state.resolvedTheme = resolvedTheme;
        }
      },
    }
  )
);

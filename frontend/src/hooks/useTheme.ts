import { useEffect } from "react";
import { useThemeStore } from "@/stores/themeStore";

export function useTheme() {
  const theme = useThemeStore((s) => s.theme);
  const resolvedTheme = useThemeStore((s) => s.resolvedTheme);
  const setTheme = useThemeStore((s) => s.setTheme);
  const toggleTheme = useThemeStore((s) => s.toggleTheme);
  const initTheme = useThemeStore((s) => s.initTheme);

  // Initialize theme on first mount
  useEffect(() => {
    initTheme();
  }, [initTheme]);

  return { theme, resolvedTheme, setTheme, toggleTheme };
}

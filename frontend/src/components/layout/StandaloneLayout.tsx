import type { ReactNode } from "react";
import { ThemeToggle } from "./ThemeToggle";

/**
 * Chrome for pages that stand on their own, outside the dashboard. Unlike `AppLayout` it
 * creates no session and opens no WebSocket, which a visitor who only wants to report
 * abuse should not be given.
 */
export function StandaloneLayout({ children }: { children: ReactNode }) {
  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <header className="flex h-[50px] shrink-0 items-center justify-between border-b border-default-200 bg-white px-4 shadow-sm dark:bg-zinc-800 md:px-5">
        <a href="/" className="flex items-center">
          <img src="/logo.svg" alt="requestrepo" className="h-5" />
        </a>
        <ThemeToggle />
      </header>
      <main className="mx-auto flex w-full max-w-3xl flex-1 flex-col gap-4 p-4 md:p-6">
        {children}
      </main>
    </div>
  );
}

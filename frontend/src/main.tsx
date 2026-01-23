import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { HeroUIProvider } from "@heroui/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { ReactQueryDevtools } from "@tanstack/react-query-devtools";
import { BrowserRouter } from "react-router-dom";
import { Toaster } from "sonner";
import * as Sentry from "@sentry/react";
import { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import App from "./App";
import "./index.css";
import "flag-icons/css/flag-icons.min.css";

// Use locally bundled Monaco instead of CDN (jsdelivr is blocked in some regions)
loader.config({ monaco });

// Pre-initialize Monaco - catches init errors at the source
loader.init().catch(() => {
  // Silently handle initialization errors (e.g., if canceled during navigation)
});

// Initialize Sentry for error tracking (if DSN is configured at runtime)
// Config is injected into index.html by the backend at runtime
declare global {
  interface Window {
    __CONFIG__?: {
      DOMAIN?: string;
      SENTRY_DSN_FRONTEND?: string;
    };
  }
}

if (window.__CONFIG__?.SENTRY_DSN_FRONTEND) {
  Sentry.init({
    dsn: window.__CONFIG__.SENTRY_DSN_FRONTEND,
  });
}

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
    },
  },
});

// Suppress Monaco's expected cancellation errors during navigation
window.addEventListener("unhandledrejection", (event) => {
  const error = event.reason;
  // Monaco throws "Canceled" when editor is disposed mid-initialization
  if (error?.message === "Canceled" || error?.name === "Canceled") {
    event.preventDefault();
    return;
  }
  // Monaco throws Event objects when DOM events fail during init
  if (error instanceof Event && error.type === "error") {
    event.preventDefault();
    return;
  }
});

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <HeroUIProvider>
          <App />
          <Toaster position="bottom-right" richColors closeButton />
        </HeroUIProvider>
      </BrowserRouter>
      <ReactQueryDevtools initialIsOpen={false} />
    </QueryClientProvider>
  </StrictMode>,
);

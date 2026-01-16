import { Outlet } from "react-router-dom";
import { Topbar } from "./Topbar";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { useWebSocket } from "@/hooks/useWebSocket";
import { useAutoSession } from "@/hooks/useAutoSession";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";

export function AppLayout() {
  // Auto-create session if none exists
  useAutoSession();

  // Initialize WebSocket connection
  useWebSocket();

  // Update document title with unseen request count
  useDocumentTitle();

  return (
    <div className="h-screen w-screen overflow-hidden bg-gray-100 dark:bg-zinc-900">
      {/* Fixed Topbar - 50px height */}
      <header className="fixed left-0 right-0 top-0 z-50 h-[50px]">
        <Topbar />
      </header>

      {/* Fixed Sidebar - 200px width */}
      <aside className="fixed bottom-0 left-0 top-[50px] z-40 w-[240px] bg-gray-50 dark:bg-[#1f1f23]">
        <Sidebar />
      </aside>

      {/* Main Content - offset by topbar and sidebar */}
      <main className="ml-[240px] mt-[50px] h-[calc(100vh-50px)] overflow-hidden">
        {/* Toolbar row */}
        <Toolbar />

        {/* Content area */}
        <div className="h-[calc(100%-48px)] overflow-auto p-6">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

import { Outlet } from "react-router-dom";
import { Topbar } from "./Topbar";
import { Sidebar } from "./Sidebar";
import { Toolbar } from "./Toolbar";
import { useWebSocket } from "@/hooks/useWebSocket";
import { useAutoSession } from "@/hooks/useAutoSession";
import { useDocumentTitle } from "@/hooks/useDocumentTitle";
import { useUiStore } from "@/stores/uiStore";

export function AppLayout() {
  // Auto-create session if none exists
  useAutoSession();

  // Initialize WebSocket connection
  useWebSocket();

  // Update document title with unseen request count
  useDocumentTitle();

  const sidebarOpen = useUiStore((s) => s.sidebarOpen);
  const setSidebarOpen = useUiStore((s) => s.setSidebarOpen);

  const handleCloseSidebar = () => setSidebarOpen(false);

  return (
    <div className="h-screen w-screen overflow-hidden bg-gray-100 dark:bg-zinc-900">
      {/* Fixed Topbar - 50px height */}
      <header className="fixed left-0 right-0 top-0 z-50 h-[50px]">
        <Topbar />
      </header>

      {/* Desktop Sidebar - hidden on mobile */}
      <aside className="fixed bottom-0 left-0 top-[50px] z-40 hidden w-[240px] bg-gray-50 dark:bg-[#1f1f23] lg:block">
        <Sidebar />
      </aside>

      {/* Mobile Sidebar Overlay */}
      {sidebarOpen && (
        <>
          {/* Backdrop */}
          <div
            className="fixed inset-0 z-40 bg-black/50 lg:hidden"
            onClick={handleCloseSidebar}
          />
          {/* Sidebar */}
          <aside className="fixed bottom-0 left-0 top-0 z-50 w-[280px] bg-gray-50 dark:bg-[#1f1f23] lg:hidden">
            <Sidebar onClose={handleCloseSidebar} />
          </aside>
        </>
      )}

      {/* Main Content - offset by topbar and sidebar (sidebar margin only on lg+) */}
      <main className="mt-[50px] flex h-[calc(100vh-50px)] flex-col overflow-hidden lg:ml-[240px]">
        {/* Toolbar row */}
        <div className="shrink-0">
          <Toolbar />
        </div>

        {/* Content area */}
        <div className="min-h-0 flex-1 overflow-auto p-4 md:p-6">
          <Outlet />
        </div>
      </main>
    </div>
  );
}

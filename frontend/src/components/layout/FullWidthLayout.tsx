import { Outlet } from "react-router-dom";
import { Topbar } from "./Topbar";

export function FullWidthLayout() {
  return (
    <div className="h-screen w-screen overflow-hidden bg-gray-100 dark:bg-zinc-900">
      <header className="fixed left-0 right-0 top-0 z-50 h-[50px]">
        <Topbar />
      </header>
      <main className="mt-[50px] flex h-[calc(100vh-50px)] flex-col overflow-auto p-4 md:p-6">
        <Outlet />
      </main>
    </div>
  );
}

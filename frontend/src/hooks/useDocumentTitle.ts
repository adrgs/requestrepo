import { useEffect } from "react";
import { useRequestStore } from "@/stores/requestStore";
import { useSessionStore } from "@/stores/sessionStore";
import { useUiStore } from "@/stores/uiStore";

export function useDocumentTitle() {
  const requests = useRequestStore((s) => s.requests);
  const sessions = useSessionStore((s) => s.sessions);
  const visitedRequests = useUiStore((s) => s.visitedRequests);

  useEffect(() => {
    // Use dynamic hostname (e.g., "requestrepo.com" or "localhost:5173")
    const domain = window.location.host;
    const baseTitle = `Dashboard - ${domain}`;

    // Count unseen requests across all sessions
    let totalUnseen = 0;
    for (const session of sessions) {
      const sessionRequests = requests[session.subdomain] ?? [];
      const visited = visitedRequests[session.subdomain] ?? {};
      const unseenCount = sessionRequests.filter((r) => !visited[r._id]).length;
      totalUnseen += unseenCount;
    }

    document.title = totalUnseen > 0 ? `(${totalUnseen}) ${baseTitle}` : baseTitle;
  }, [requests, sessions, visitedRequests]);
}

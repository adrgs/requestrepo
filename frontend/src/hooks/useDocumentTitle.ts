import { useEffect, useCallback } from "react";
import { AppSession } from "../types/app-types";

export function useDocumentTitle(sessions: Record<string, AppSession>): {
  updateDocumentTitle: () => void;
} {
  const updateDocumentTitle = useCallback(() => {
    let totalUnseen = 0;
    Object.values(sessions).forEach((session) => {
      let unseenCount = 0;
      Object.keys(session.requests).forEach((key) => {
        if (session.requests[key].new && !session.visited[key]) {
          unseenCount++;
        }
      });
      totalUnseen += unseenCount;
    });

    document.title =
      totalUnseen > 0 ? `(${totalUnseen}) RequestRepo` : "RequestRepo";
  }, [sessions]);

  useEffect(() => {
    updateDocumentTitle();
  }, [updateDocumentTitle]);

  return { updateDocumentTitle };
}

import { useEffect } from "react";

/** Sets the browser tab title for a page outside the dashboard, which names itself. */
export function usePageTitle(title: string) {
  useEffect(() => {
    document.title = `${title} - ${window.location.host}`;
  }, [title]);
}

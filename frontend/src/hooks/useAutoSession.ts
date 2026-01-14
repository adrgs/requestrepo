import { useEffect, useRef } from "react";
import { useSessionStore } from "@/stores/sessionStore";
import { useUiStore } from "@/stores/uiStore";
import { useAuthStore } from "@/stores/authStore";
import { apiClient, isAdminRequiredError } from "@/api/client";

/**
 * Automatically creates a session if none exists.
 * Handles shared session tokens and request links from URL params.
 * Handles API failures gracefully - the app works in offline mode.
 */
export function useAutoSession() {
  const sessions = useSessionStore((s) => s.sessions);
  const addSession = useSessionStore((s) => s.addSession);
  const setActiveSession = useSessionStore((s) => s.setActiveSession);
  const selectRequest = useUiStore((s) => s.selectRequest);
  const setSharedRequest = useUiStore((s) => s.setSharedRequest);
  const isCreating = useRef(false);
  const urlHandled = useRef(false);
  const isHandlingShareUrl = useRef(false);

  // Handle URL params (share token and/or request link)
  useEffect(() => {
    if (urlHandled.current) return;

    const urlParams = new URLSearchParams(window.location.search);
    const sharedToken = urlParams.get("share");
    const requestParam = urlParams.get("request");

    // Nothing to handle
    if (!sharedToken && !requestParam) return;

    urlHandled.current = true;
    // Mark that we're handling a share URL (prevents auto-create session)
    isHandlingShareUrl.current = true;

    // Handle shared session token (full session share)
    if (sharedToken) {
      try {
        // Decode JWT to get subdomain
        const parts = sharedToken.split(".");
        if (parts.length >= 2) {
          const payload = JSON.parse(atob(parts[1]));
          const subdomain = payload.subdomain;

          // Check if session already exists (use getState for fresh data)
          const currentSessions = useSessionStore.getState().sessions;
          if (
            subdomain &&
            !currentSessions.some((s) => s.subdomain === subdomain)
          ) {
            addSession({
              subdomain,
              token: sharedToken,
              createdAt: new Date().toISOString(),
            });
          }
          setActiveSession(subdomain);
        }
      } catch (e) {
        console.error("Invalid share token:", e);
      }
      // Clean URL
      window.history.replaceState({}, "", window.location.pathname);
      return;
    }

    // Handle shared request link (share token from API)
    if (requestParam) {
      // Fetch the shared request asynchronously
      apiClient
        .getSharedRequest(requestParam)
        .then((requestData) => {
          if (requestData) {
            const subdomain = requestData.uid;

            // Get FRESH sessions from store, not stale closure
            const currentSessions = useSessionStore.getState().sessions;
            const existingSession = currentSessions.find(
              (s) => s.subdomain === subdomain,
            );

            if (existingSession) {
              // User has the session, just select the request
              setActiveSession(subdomain);
              selectRequest(requestData._id);
            } else {
              // User doesn't have the session, show the shared request
              setSharedRequest(requestData);
            }
          } else {
            console.error("Failed to load shared request");
          }
        })
        .catch((e) => {
          console.error("Invalid request share token:", e);
        });
      // Clean URL
      window.history.replaceState({}, "", window.location.pathname);
    }
  }, [addSession, setActiveSession, selectRequest, setSharedRequest]);

  // Get auth store actions
  const setShowAuthOverlay = useAuthStore((s) => s.setShowAuthOverlay);
  const setBackendOffline = useAuthStore((s) => s.setBackendOffline);

  // Auto-create session if none exists (runs independently of share URL handling)
  useEffect(() => {
    // Skip if we already have sessions, are creating one, or handling a share URL
    if (sessions.length > 0 || isCreating.current || isHandlingShareUrl.current)
      return;

    isCreating.current = true;

    // Create session in background - don't block UI
    apiClient
      .createSession()
      .then((response) => {
        setBackendOffline(false);
        addSession({
          subdomain: response.subdomain,
          token: response.token,
          createdAt: new Date().toISOString(),
        });
      })
      .catch((error) => {
        if (isAdminRequiredError(error)) {
          // Show auth overlay - user needs to enter admin password
          setBackendOffline(false);
          setShowAuthOverlay(true);
        } else {
          // Backend is offline or unreachable
          setBackendOffline(true);
          console.log("Auto-session creation failed - backend offline");
        }
      })
      .finally(() => {
        isCreating.current = false;
      });
  }, [sessions.length, addSession, setShowAuthOverlay, setBackendOffline]);
}

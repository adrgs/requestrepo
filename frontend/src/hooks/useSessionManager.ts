import { useState, useEffect, useCallback } from "react";
import { Utils } from "../utils";
import { AppSession, SessionData } from "../types/app-types";
import { WebSocketSession } from "../services/websocket-service";

export function useSessionManager(
  initialState: Pick<AppSession, "subdomain" | "token"> | null = null,
): {
  sessions: Record<string, AppSession>;
  activeSession: string;
  activeSessionData: AppSession | null;
  initializeSessions: () => Promise<void>;
  handleNewURL: () => Promise<{ token: string; subdomain: string }>;
  getWebSocketSessions: () => WebSocketSession[];
} {
  const [sessions, setSessions] = useState<Record<string, AppSession>>({});
  const [activeSession, setActiveSession] = useState<string>("");

  const activeSessionData = activeSession ? sessions[activeSession] : null;

  const initializeSessions = useCallback(async () => {
    const allSessions = Utils.getAllSessions();
    if (allSessions.length === 0) {
      try {
        const { subdomain, token } = await Utils.getRandomSubdomain();

        const newSession: SessionData = {
          subdomain,
          token,
          createdAt: new Date().toISOString(),
          unseenRequests: 0,
        };

        localStorage.setItem("sessions", JSON.stringify([newSession]));
        localStorage.setItem("selectedSessionIndex", "0");

        setSessions({
          [subdomain]: {
            url: `${subdomain}.${Utils.siteUrl}`,
            domain: Utils.siteUrl,
            subdomain: subdomain,
            httpRequests: [],
            dnsRequests: [],
            timestamp: null,
            requests: {},
            visited: {},
            selectedRequest: null,
            token: token,
          },
        });
        setActiveSession(subdomain);
      } catch (error) {
        console.error("Failed to create initial session:", error);
      }
    } else {
      const sessionsMap: Record<string, AppSession> = {};
      allSessions.forEach((session) => {
        sessionsMap[session.subdomain] = {
          url: `${session.subdomain}.${Utils.siteUrl}`,
          domain: Utils.siteUrl,
          subdomain: session.subdomain,
          httpRequests: [],
          dnsRequests: [],
          timestamp: null,
          requests: {},
          visited: {},
          selectedRequest: null,
          token: session.token,
        };
      });

      const activeSessionSubdomain =
        sessionStorage.getItem("activeSessionSubdomain") ||
        Utils.getActiveSession()?.subdomain ||
        allSessions[0].subdomain;

      const finalActiveSession = sessionsMap[activeSessionSubdomain]
        ? activeSessionSubdomain
        : allSessions[0].subdomain;

      setSessions(sessionsMap);
      setActiveSession(finalActiveSession);
    }
  }, []);

  const handleNewURL = useCallback(async () => {
    try {
      const { subdomain, token } = await Utils.getRandomSubdomain();

      const allSessions = Utils.getAllSessions();
      const sessionIndex = allSessions.length;
      allSessions.push({
        subdomain,
        token,
        createdAt: new Date().toISOString(),
        unseenRequests: 0,
      });

      localStorage.setItem("sessions", JSON.stringify(allSessions));
      localStorage.setItem("selectedSessionIndex", sessionIndex.toString());
      sessionStorage.setItem("activeSessionSubdomain", subdomain);

      setSessions((prevSessions) => {
        const newSessions = {
          ...prevSessions,
          [subdomain]: {
            url: `${subdomain}.${Utils.siteUrl}`,
            domain: Utils.siteUrl,
            subdomain: subdomain,
            httpRequests: [],
            dnsRequests: [],
            timestamp: null,
            requests: {},
            visited: {},
            selectedRequest: null,
            token: token,
          },
        };

        return newSessions;
      });

      setActiveSession(subdomain);
      return { token, subdomain };
    } catch (error) {
      console.error("Failed to create new URL:", error);
      throw error;
    }
  }, []);

  const getWebSocketSessions = useCallback((): WebSocketSession[] => {
    return Object.entries(sessions)
      .filter(([, session]) => session && session.token)
      .map(([subdomain, session]) => ({
        token: session.token,
        subdomain,
      }));
  }, [sessions]);

  useEffect(() => {
    if (initialState) {
      setSessions({
        [initialState.subdomain]: {
          url: `${initialState.subdomain}.${Utils.siteUrl}`,
          domain: Utils.siteUrl,
          subdomain: initialState.subdomain,
          httpRequests: [],
          dnsRequests: [],
          timestamp: null,
          requests: {},
          visited: {},
          selectedRequest: null,
          token: initialState.token,
        },
      });
      setActiveSession(initialState.subdomain);
    } else {
      initializeSessions();
    }
  }, [initialState, initializeSessions]);

  return {
    sessions,
    activeSession,
    activeSessionData,
    initializeSessions,
    handleNewURL,
    getWebSocketSessions,
  };
}

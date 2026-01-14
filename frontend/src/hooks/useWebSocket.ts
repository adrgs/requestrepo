import { useEffect, useRef, useCallback } from "react";
import { useSessionStore } from "@/stores/sessionStore";
import { useRequestStore } from "@/stores/requestStore";
import type { WebSocketServerMessage, Session } from "@/types";

const RECONNECT_DELAY = 2500;
const HEARTBEAT_INTERVAL = 30000;

export function useWebSocket() {
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const heartbeatIntervalRef = useRef<ReturnType<typeof setInterval> | null>(
    null,
  );
  // Track tokens being registered to correlate with errors
  const pendingTokensRef = useRef<string[]>([]);
  // Track validated subdomains
  const validatedSubdomainsRef = useRef<Set<string>>(new Set());
  // Store connect function ref to avoid circular reference
  const connectRef = useRef<() => void>(() => {});
  // Store sessions in ref to avoid dependency in handleMessage
  const sessionsRef = useRef<Session[]>([]);

  const sessions = useSessionStore((s) => s.sessions);
  const removeSession = useSessionStore((s) => s.removeSession);
  const setRequests = useRequestStore((s) => s.setRequests);
  const addRequest = useRequestStore((s) => s.addRequest);
  const removeRequest = useRequestStore((s) => s.removeRequest);
  const clearRequests = useRequestStore((s) => s.clearRequests);

  // Keep sessions ref updated in an effect
  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  const getWsUrl = useCallback(() => {
    const protocol = window.location.protocol === "https:" ? "wss" : "ws";
    return `${protocol}://${window.location.host}/api/v2/ws`;
  }, []);

  const handleMessage = useCallback(
    (event: MessageEvent) => {
      try {
        const message = JSON.parse(event.data) as WebSocketServerMessage;

        switch (message.cmd) {
          case "connected":
            validatedSubdomainsRef.current.add(message.subdomain);
            // Remove from pending - session is valid
            pendingTokensRef.current = [];
            break;

          case "request":
            addRequest(message.subdomain, message.data);
            break;

          case "requests":
            setRequests(message.subdomain, message.data);
            break;

          case "deleted":
            removeRequest(message.subdomain, message.data._id);
            break;

          case "cleared":
            clearRequests(message.subdomain);
            break;

          case "pong":
            // Connection is alive
            break;

          case "error":
            // If invalid token, remove the session that caused it
            if (
              message.code === "invalid_token" &&
              pendingTokensRef.current.length > 0
            ) {
              const badToken = pendingTokensRef.current.shift();
              const badSession = sessionsRef.current.find(
                (s) => s.token === badToken,
              );
              if (badSession) {
                removeSession(badSession.subdomain);
              }
            }
            break;
        }
      } catch (error) {
        console.error("WebSocket message parse error:", error);
      }
    },
    [addRequest, setRequests, removeRequest, clearRequests, removeSession],
  );

  const connect = useCallback(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) return;
    if (sessions.length === 0) return;

    const ws = new WebSocket(getWsUrl());
    wsRef.current = ws;

    ws.onopen = () => {
      validatedSubdomainsRef.current.clear();

      // Register all sessions - track pending tokens
      for (const session of sessions) {
        pendingTokensRef.current.push(session.token);
        ws.send(
          JSON.stringify({
            cmd: "connect",
            token: session.token,
          }),
        );
      }

      // Start heartbeat
      heartbeatIntervalRef.current = setInterval(() => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ cmd: "ping" }));
        }
      }, HEARTBEAT_INTERVAL);
    };

    ws.onmessage = handleMessage;

    ws.onclose = (event) => {
      if (heartbeatIntervalRef.current) {
        clearInterval(heartbeatIntervalRef.current);
        heartbeatIntervalRef.current = null;
      }

      // Reconnect unless intentionally closed
      if (event.code !== 1000) {
        reconnectTimeoutRef.current = setTimeout(() => {
          connectRef.current();
        }, RECONNECT_DELAY);
      }
    };

    ws.onerror = () => {
      ws.close();
    };
  }, [getWsUrl, sessions, handleMessage]);

  // Keep connect ref up to date
  useEffect(() => {
    connectRef.current = connect;
  }, [connect]);

  const disconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }
    if (heartbeatIntervalRef.current) {
      clearInterval(heartbeatIntervalRef.current);
      heartbeatIntervalRef.current = null;
    }
    wsRef.current?.close(1000);
    wsRef.current = null;
  }, []);

  const registerSession = useCallback((token: string) => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      pendingTokensRef.current.push(token);
      wsRef.current.send(
        JSON.stringify({
          cmd: "connect",
          token,
        }),
      );
    }
  }, []);

  // Connect on mount, reconnect when sessions change
  useEffect(() => {
    connect();

    const handleVisibility = () => {
      if (document.visibilityState === "visible") {
        if (wsRef.current?.readyState !== WebSocket.OPEN) {
          connect();
        }
      }
    };

    document.addEventListener("visibilitychange", handleVisibility);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibility);
      disconnect();
    };
  }, [connect, disconnect]);

  // Register new sessions
  useEffect(() => {
    if (wsRef.current?.readyState === WebSocket.OPEN) {
      for (const session of sessions) {
        registerSession(session.token);
      }
    }
  }, [sessions, registerSession]);

  return { registerSession, disconnect };
}

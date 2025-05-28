import { useRef, useEffect, useCallback } from "react";

export interface WebSocketSession {
  token: string;
  subdomain: string;
}

export interface PingMessage {
  cmd: "ping";
}

export interface PongMessage {
  cmd: "pong";
}

export interface RegisterSessionsMessage {
  cmd: "register_sessions";
  sessions: WebSocketSession[];
}

export interface RequestMessage {
  cmd: "request";
  subdomain: string;
  request: Record<string, string | number | boolean | null>;
}

export type WebSocketMessage =
  | PingMessage
  | PongMessage
  | RegisterSessionsMessage
  | RequestMessage;

export interface WebSocketState {
  isConnected: boolean;
  isConnecting: boolean;
  lastPongTime: number;
  reconnectAttempts: number;
  connectionError: boolean;
}

export interface WebSocketServiceProps {
  url: string;
  onMessage: (event: MessageEvent, subdomain: string) => void;
  onOpen?: () => void;
  sessions: WebSocketSession[];
  debug?: boolean;
}

const MAX_RECONNECT_DELAY = 120000; // 2 minutes
const HEARTBEAT_INTERVAL = 600000; // 10 minutes (increased from 5 minutes)
const PONG_TIMEOUT = 60000; // 60 seconds (increased from 30 seconds)
const MAX_RECONNECT_ATTEMPTS = 1; // Only one reconnection attempt
const DEBOUNCE_VISIBILITY_CHANGE = 30000; // 30 seconds debounce for visibility change (increased from 10 seconds)

export function useWebSocketService({
  url,
  onMessage,
  onOpen,
  sessions,
  debug = false,
}: WebSocketServiceProps): {
  state: WebSocketState;
  sendMessage: (message: WebSocketMessage) => void;
  registerSessions: (sessions: WebSocketSession[]) => void;
} {
  const hasRegisteredRef = useRef<boolean>(false);
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const heartbeatIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const visibilityTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const sessionsRef = useRef<WebSocketSession[]>(sessions);
  const stateRef = useRef<WebSocketState>({
    isConnected: false,
    isConnecting: false,
    lastPongTime: Date.now(),
    reconnectAttempts: 0,
    connectionError: false,
  });

  const log = useCallback(
    (message: string, ...args: unknown[]) => {
      if (debug) {
        console.log(`[WebSocketService] ${message}`, ...args);
      }
    },
    [debug],
  );

  const resetState = useCallback(() => {
    stateRef.current = {
      ...stateRef.current,
      isConnected: false,
      isConnecting: false,
    };
  }, []);

  const sendMessage = useCallback(
    (message: WebSocketMessage) => {
      if (socketRef.current?.readyState === WebSocket.OPEN) {
        try {
          socketRef.current.send(JSON.stringify(message));
          log("Sent message", message);
        } catch (error) {
          console.error("Error sending message:", error);
        }
      } else {
        log("Cannot send message, socket not open", message);
      }
    },
    [log],
  );

  const registerSessions = useCallback(
    (sessions: WebSocketSession[]) => {
      if (sessions.length === 0) {
        return;
      }

      const currentSessions = sessionsRef.current;

      const sessionsChanged =
        sessions.length !== currentSessions.length ||
        sessions.some(
          (s, i) =>
            s.token !== currentSessions[i]?.token ||
            s.subdomain !== currentSessions[i]?.subdomain,
        );

      if (sessionsChanged) {
        sessionsRef.current = sessions;

        if (
          socketRef.current?.readyState === WebSocket.OPEN &&
          !hasRegisteredRef.current
        ) {
          sendMessage({
            cmd: "register_sessions",
            sessions,
          });
          hasRegisteredRef.current = true;
          log("Registered sessions", sessions);
        }
      }
    },
    [sendMessage, log],
  );

  const sendHeartbeat = useCallback(() => {
    if (socketRef.current?.readyState === WebSocket.OPEN) {
      try {
        sendMessage({ cmd: "ping" });

        const timeSinceLastPong = Date.now() - stateRef.current.lastPongTime;
        if (timeSinceLastPong > PONG_TIMEOUT) {
          log("WebSocket connection stale, reconnecting...");
          socketRef.current.close();
        }
      } catch (error) {
        console.error("Error sending heartbeat:", error);
        if (socketRef.current) {
          socketRef.current.close();
        }
      }
    }
  }, [sendMessage, log]);

  const getReconnectDelay = useCallback(() => {
    const baseDelay = Math.min(
      5000 * Math.pow(2, stateRef.current.reconnectAttempts),
      MAX_RECONNECT_DELAY,
    );
    return baseDelay * (0.8 + Math.random() * 0.4);
  }, []);

  const connect = useCallback(() => {
    if (stateRef.current.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
      log(
        `Max reconnection attempts (${MAX_RECONNECT_ATTEMPTS}) reached, giving up`,
      );
      stateRef.current.connectionError = true;
      return;
    }

    // Prevent multiple simultaneous connection attempts
    if (stateRef.current.isConnecting) {
      log("Already connecting, skipping connect call");
      return;
    }

    if (socketRef.current?.readyState === WebSocket.OPEN) {
      log("Socket already open, skipping connect call");
      return;
    }

    stateRef.current.isConnecting = true;
    log("Connecting to WebSocket", url);

    if (socketRef.current) {
      try {
        socketRef.current.close();
      } catch (err) {
        console.error("Error closing existing socket:", err);
      }
    }

    if (heartbeatIntervalRef.current) {
      clearInterval(heartbeatIntervalRef.current);
      heartbeatIntervalRef.current = null;
    }

    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current);
      reconnectTimeoutRef.current = null;
    }

    try {
      const socket = new WebSocket(url);
      socketRef.current = socket;

      socket.onopen = () => {
        log("WebSocket connected");
        stateRef.current.isConnected = true;
        stateRef.current.isConnecting = false;
        stateRef.current.lastPongTime = Date.now();
        stateRef.current.reconnectAttempts = 0;
        stateRef.current.connectionError = false;
        hasRegisteredRef.current = false; // Reset registration flag on new connection

        if (sessionsRef.current.length > 0) {
          sendMessage({
            cmd: "register_sessions",
            sessions: sessionsRef.current,
          });
          hasRegisteredRef.current = true;
          log("Registered sessions on connect", sessionsRef.current);
        }

        heartbeatIntervalRef.current = setInterval(
          sendHeartbeat,
          HEARTBEAT_INTERVAL,
        );

        if (onOpen) {
          onOpen();
        }
      };

      socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data as string);

          if (data.cmd === "pong") {
            stateRef.current.lastPongTime = Date.now();
            return;
          }

          onMessage(event, data.subdomain || "");
        } catch (err) {
          console.error("Error handling WebSocket message:", err);
        }
      };

      socket.onclose = (event) => {
        log("WebSocket closed", event.code, event.reason);
        resetState();
        hasRegisteredRef.current = false; // Reset registration flag on close

        if (heartbeatIntervalRef.current) {
          clearInterval(heartbeatIntervalRef.current);
          heartbeatIntervalRef.current = null;
        }

        if (event.code !== 1000) {
          stateRef.current.reconnectAttempts++;
          const delay = getReconnectDelay();

          log(
            `Reconnecting in ${delay}ms (attempt ${stateRef.current.reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS})`,
          );

          if (stateRef.current.reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
            reconnectTimeoutRef.current = setTimeout(connect, delay);
          } else {
            log("Max reconnection attempts reached, giving up");
            stateRef.current.connectionError = true;
          }
        }
      };

      socket.onerror = (error) => {
        console.error("WebSocket error:", error);
        stateRef.current.isConnecting = false;
      };
    } catch (error) {
      console.error("Error creating WebSocket:", error);
      stateRef.current.isConnecting = false;

      stateRef.current.reconnectAttempts++;
      const delay = getReconnectDelay();
      log(
        `Reconnecting in ${delay}ms (attempt ${stateRef.current.reconnectAttempts}/${MAX_RECONNECT_ATTEMPTS})`,
      );

      if (stateRef.current.reconnectAttempts < MAX_RECONNECT_ATTEMPTS) {
        reconnectTimeoutRef.current = setTimeout(connect, delay);
      } else {
        log("Max reconnection attempts reached, giving up");
        stateRef.current.connectionError = true;
      }
    }
  }, [
    url,
    onOpen,
    onMessage,
    sendMessage,
    resetState,
    getReconnectDelay,
    log,
    sendHeartbeat,
  ]);

  useEffect(() => {
    connect();

    const handleVisibilityChange = () => {
      // Prevent multiple visibility change handlers from firing
      if (visibilityTimeoutRef.current) {
        clearTimeout(visibilityTimeoutRef.current);
      }

      visibilityTimeoutRef.current = setTimeout(() => {
        if (
          document.visibilityState === "visible" &&
          stateRef.current.reconnectAttempts < MAX_RECONNECT_ATTEMPTS &&
          (!socketRef.current ||
            socketRef.current.readyState === WebSocket.CLOSED ||
            socketRef.current.readyState === WebSocket.CLOSING) &&
          Date.now() - stateRef.current.lastPongTime > PONG_TIMEOUT * 2
        ) {
          log("Page became visible after long absence, reconnecting if needed");
          if (!stateRef.current.isConnected && !stateRef.current.isConnecting) {
            connect();
          }
        }
      }, DEBOUNCE_VISIBILITY_CHANGE);
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);

      if (heartbeatIntervalRef.current) {
        clearInterval(heartbeatIntervalRef.current);
      }

      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }

      if (visibilityTimeoutRef.current) {
        clearTimeout(visibilityTimeoutRef.current);
      }

      if (socketRef.current) {
        socketRef.current.close(1000); // Normal closure
      }
    };
  }, [connect, log]);

  useEffect(() => {
    const currentSessions = sessionsRef.current;
    const sessionsChanged =
      sessions.length !== currentSessions.length ||
      sessions.some(
        (s, i) =>
          s.token !== currentSessions[i]?.token ||
          s.subdomain !== currentSessions[i]?.subdomain,
      );

    if (sessionsChanged) {
      sessionsRef.current = sessions;
    }
  }, [sessions]);

  return {
    state: stateRef.current,
    sendMessage,
    registerSessions,
  };
}

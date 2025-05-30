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

const MAX_RECONNECT_DELAY = 7200000; // 2 hours
const HEARTBEAT_INTERVAL = 7200000; // 2 hours (significantly increased to reduce connection frequency)
const PONG_TIMEOUT = 3600000; // 1 hour (increased to reduce false reconnections)
const MAX_RECONNECT_ATTEMPTS = 1; // Limited to prevent excessive reconnection attempts
const DEBOUNCE_VISIBILITY_CHANGE = 7200000; // 2 hours (increased to reduce visibility change reconnections)
const CONNECTION_COOLDOWN = 7200000; // 2 hours (increased to prevent rapid reconnection attempts)
const RECONNECT_AFTER_VISIBILITY_TIMEOUT = 14400000; // 4 hours (only reconnect if page was hidden for a long time)

export const useWebSocketService = ({
  url,
  onMessage,
  onOpen,
  sessions,
  debug = false,
}: WebSocketServiceProps): {
  state: WebSocketState;
  sendMessage: (message: WebSocketMessage) => void;
  registerSessions: (sessions: WebSocketSession[]) => void;
} => {
  const hasRegisteredRef = useRef<boolean>(false);
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const heartbeatIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const visibilityTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const lastConnectionAttemptRef = useRef<number>(0);
  const sessionsRef = useRef<WebSocketSession[]>(sessions);
  const stateRef = useRef<WebSocketState>({
    isConnected: false,
    isConnecting: false,
    lastPongTime: Date.now(),
    reconnectAttempts: 0,
    connectionError: false,
  });

  const log = useCallback(
    (message: string, ...args: (string | number | object)[]) => {
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
      10000 * Math.pow(2, stateRef.current.reconnectAttempts),
      MAX_RECONNECT_DELAY,
    );
    return baseDelay * (0.8 + Math.random() * 0.4);
  }, []);

  const connect = useCallback(() => {
    // Enforce connection cooldown to prevent rapid reconnection attempts
    const now = Date.now();
    const timeSinceLastAttempt = now - lastConnectionAttemptRef.current;

    if (timeSinceLastAttempt < CONNECTION_COOLDOWN) {
      log(
        `Connection attempt too soon (${timeSinceLastAttempt}ms), enforcing cooldown`,
      );
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      reconnectTimeoutRef.current = setTimeout(
        connect,
        CONNECTION_COOLDOWN - timeSinceLastAttempt,
      );
      return;
    }

    lastConnectionAttemptRef.current = now;

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
      let wsUrl = url;
      if (url.startsWith("/")) {
        const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
        const host = window.location.host;
        wsUrl = `${protocol}//${host}${url}`;
      }

      if (sessionsRef.current.length > 0) {
        const session = sessionsRef.current[0]; // Use only the first session token
        const separator = wsUrl.includes("?") ? "&" : "?";
        wsUrl = `${wsUrl}${separator}token=${encodeURIComponent(session.token)}`;
        log("Connecting with token in query parameter", wsUrl);
      }

      const socket = new WebSocket(wsUrl);
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
    if (sessions.length > 0) {
      connect();
    }

    const handleVisibilityChange = () => {
      // Prevent multiple visibility change handlers from firing
      if (visibilityTimeoutRef.current) {
        clearTimeout(visibilityTimeoutRef.current);
      }

      const visibilityChangeTime = Date.now();
      const lastVisibleTime = stateRef.current.lastPongTime || 0;
      const timeHidden = visibilityChangeTime - lastVisibleTime;

      if (timeHidden < RECONNECT_AFTER_VISIBILITY_TIMEOUT) {
        log(`Page was hidden for only ${timeHidden}ms, skipping reconnection`);
        return;
      }

      visibilityTimeoutRef.current = setTimeout(() => {
        if (
          document.visibilityState === "visible" &&
          stateRef.current.reconnectAttempts < MAX_RECONNECT_ATTEMPTS &&
          (!socketRef.current ||
            socketRef.current.readyState === WebSocket.CLOSED ||
            socketRef.current.readyState === WebSocket.CLOSING) &&
          Date.now() - stateRef.current.lastPongTime > PONG_TIMEOUT * 3 && // Increased threshold
          sessions.length > 0 // Only reconnect if we have sessions
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
  }, [connect, log, sessions]);

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
};

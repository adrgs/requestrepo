import { useRef, useEffect, useCallback } from "react";

export interface WebSocketSession {
  token: string;
  subdomain: string;
}

export interface WebSocketMessage {
  cmd: string;
  [key: string]: unknown;
}

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

const MAX_RECONNECT_DELAY = 60000; // 60 seconds (increased from 30 seconds)
const HEARTBEAT_INTERVAL = 60000; // 60 seconds (increased from 30 seconds)
const PONG_TIMEOUT = 15000; // 15 seconds (increased from 10 seconds)
const MAX_RECONNECT_ATTEMPTS = 5; // Limit reconnection attempts

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
  const socketRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const heartbeatIntervalRef = useRef<NodeJS.Timeout | null>(null);
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
      sessionsRef.current = sessions;
      if (
        sessions.length > 0 &&
        socketRef.current?.readyState === WebSocket.OPEN
      ) {
        sendMessage({
          cmd: "register_sessions",
          sessions,
        });
        log("Registered sessions", sessions);
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
      2000 * Math.pow(2, stateRef.current.reconnectAttempts),
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

        if (sessionsRef.current.length > 0) {
          sendMessage({
            cmd: "register_sessions",
            sessions: sessionsRef.current,
          });
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
      if (
        document.visibilityState === "visible" &&
        stateRef.current.reconnectAttempts < MAX_RECONNECT_ATTEMPTS &&
        (!socketRef.current ||
          socketRef.current.readyState === WebSocket.CLOSED ||
          socketRef.current.readyState === WebSocket.CLOSING)
      ) {
        log("Page became visible, reconnecting if needed");
        if (!stateRef.current.isConnected && !stateRef.current.isConnecting) {
          connect();
        }
      }
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

      if (socketRef.current) {
        socketRef.current.close(1000); // Normal closure
      }
    };
  }, [url, connect, log]);

  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  return {
    state: stateRef.current,
    sendMessage,
    registerSessions,
  };
}

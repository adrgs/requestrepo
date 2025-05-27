import React, { useState, useEffect, useRef, useCallback } from "react";
import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { AppTopbar } from "./components/topbar";
import { AppSidebar } from "./components/sidebar";
import { RequestsPage } from "./components/requests-page";
import { EditResponsePage } from "./components/edit-response-page";
import { DnsSettingsPage } from "./components/dns-settings-page";
import { Toolbar } from "primereact/toolbar";
import { Button } from "primereact/button";
import { InputText } from "primereact/inputtext";
import { Utils } from "./utils";
import { toast } from "react-toastify";
import "react-toastify/dist/ReactToastify.css";
import { ToastContainer } from "react-toastify";
import "primereact/resources/themes/lara-light-blue/theme.css";
import "primereact/resources/primereact.min.css";
import "primeicons/primeicons.css";
import "primeflex/primeflex.css";
import "react-toastify/dist/ReactToastify.css";
import "./app.scss";

interface Request {
  _id: string;
  type: string;
  new?: boolean;
  [key: string]: unknown;
}

interface AppSession {
  url: string;
  domain: string;
  subdomain: string;
  httpRequests: Request[];
  dnsRequests: Request[];
  timestamp: string | null;
  requests: Record<string, Request>;
  visited: Record<string, boolean>;
  selectedRequest: string | null;
  token: string;
  dnsRecords?: Array<Record<string, unknown>>;
}

interface AppState {
  layoutMode: string;
  layoutColorMode: string;
  staticMenuInactive: boolean;
  overlayMenuActive: boolean;
  mobileMenuActive: boolean;
  sessions: Record<string, AppSession>;
  activeSession: string;
  searchValue: string;
  response: {
    raw: string;
    headers: Array<{ key: string; value: string }>;
    status_code: number;
    fetched: boolean;
  };
  dnsRecords: Array<Record<string, unknown>>;
  dnsFetched: boolean;
}

interface WebSocketProps {
  ws_url: string;
  onUpdate: (event: MessageEvent, subdomain: string) => void;
  onOpen: () => void;
  sessions: Record<string, AppSession>;
  websocketRef: React.RefObject<WebSocket | null>;
}

function useWebSocket({
  ws_url,
  onUpdate,
  onOpen,
  sessions,
  websocketRef,
}: WebSocketProps): void {
  const reconnectTimeoutRef = useRef<NodeJS.Timeout | null>(null);
  const isConnectingRef = useRef<boolean>(false);
  const sessionsRef = useRef<Record<string, AppSession>>(sessions);
  const onOpenRef = useRef<() => void>(onOpen);
  const heartbeatIntervalRef = useRef<NodeJS.Timeout | null>(null);
  const lastPongTimeRef = useRef<number>(Date.now());

  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  useEffect(() => {
    onOpenRef.current = onOpen;
  }, [onOpen]);

  const sendHeartbeat = useCallback(() => {
    if (websocketRef.current?.readyState === WebSocket.OPEN) {
      try {
        websocketRef.current.send(JSON.stringify({ cmd: "ping" }));

        const timeSinceLastPong = Date.now() - lastPongTimeRef.current;
        if (timeSinceLastPong > 10000) {
          console.warn("WebSocket connection may be stale, reconnecting...");
          websocketRef.current.close();
        }
      } catch (error) {
        console.error("Error sending heartbeat:", error);
        if (websocketRef.current) {
          websocketRef.current.close();
        }
      }
    }
  }, []);

  const connectWebSocket = useCallback(() => {
    if (
      isConnectingRef.current ||
      websocketRef.current?.readyState === WebSocket.CONNECTING
    ) {
      return;
    }

    if (websocketRef.current?.readyState === WebSocket.OPEN) {
      return;
    }

    isConnectingRef.current = true;

    if (websocketRef.current) {
      try {
        websocketRef.current.close();
      } catch (err) {
        console.error("Error closing websocket:", err);
      }
    }

    if (heartbeatIntervalRef.current) {
      clearInterval(heartbeatIntervalRef.current);
      heartbeatIntervalRef.current = null;
    }

    const socket = new WebSocket(ws_url);
    websocketRef.current = socket;

    socket.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data as string);

        if (data.cmd === "pong") {
          lastPongTimeRef.current = Date.now();
          return;
        }

        onUpdate(event, data.subdomain || Utils.subdomain);
      } catch (err) {
        console.error("Error handling websocket message:", err);
      }
    };

    socket.onopen = () => {
      isConnectingRef.current = false;
      lastPongTimeRef.current = Date.now();

      try {
        const sessionTokens = Object.entries(sessionsRef.current)
          .filter(([, session]) => session && session.token)
          .map(([subdomain, session]) => ({
            token: session.token,
            subdomain: subdomain,
          }));

        if (sessionTokens.length > 0) {
          socket.send(
            JSON.stringify({
              cmd: "register_sessions",
              sessions: sessionTokens,
            }),
          );
        }

        heartbeatIntervalRef.current = setInterval(sendHeartbeat, 5000); // 5 seconds

        if (onOpenRef.current) {
          onOpenRef.current();
        }
      } catch (err) {
        console.error("Error in websocket onopen:", err);
      }
    };

    socket.onclose = (event) => {
      isConnectingRef.current = false;

      if (heartbeatIntervalRef.current) {
        clearInterval(heartbeatIntervalRef.current);
        heartbeatIntervalRef.current = null;
      }

      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }

      if (websocketRef.current === socket && event.code !== 1000) {
        reconnectTimeoutRef.current = setTimeout(connectWebSocket, 2500);
      }
    };

    socket.onerror = (error) => {
      console.error("WebSocket error:", error);
      isConnectingRef.current = false;
      if (websocketRef.current === socket) {
        socket.close();
      }
    };
  }, [ws_url, onUpdate, sendHeartbeat]);

  useEffect(() => {
    connectWebSocket();

    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        if (
          !websocketRef.current ||
          websocketRef.current.readyState === WebSocket.CLOSED ||
          websocketRef.current.readyState === WebSocket.CLOSING
        ) {
          isConnectingRef.current = false;
          connectWebSocket();
        } else if (websocketRef.current.readyState === WebSocket.OPEN) {
          sendHeartbeat();
        }
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      document.removeEventListener("visibilitychange", handleVisibilityChange);
      isConnectingRef.current = false;

      if (heartbeatIntervalRef.current) {
        clearInterval(heartbeatIntervalRef.current);
      }

      if (websocketRef.current) {
        websocketRef.current.close(1000); // Normal closure
      }

      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
    };
  }, [ws_url, connectWebSocket, sendHeartbeat]); // Only reconnect if ws_url changes
}

const App: React.FC = () => {
  const urlArea = useRef<HTMLInputElement | null>(null);
  const websocketRef = useRef<WebSocket | null>(null);
  const location = useLocation();
  const navigate = useNavigate();
  const [sharedRequest, setSharedRequest] = useState<string | null>(null);
  const [appState, setAppState] = useState<AppState>({
    layoutMode: "static",
    layoutColorMode: "light",
    staticMenuInactive: false,
    overlayMenuActive: false,
    mobileMenuActive: false,
    sessions: {}, // Initialize empty, then update in useEffect
    activeSession: "",
    searchValue: "",
    response: { raw: "", headers: [], status_code: 200, fetched: false },
    dnsRecords: [],
    dnsFetched: false,
  });

  const updateDocumentTitle = useCallback(() => {
    let totalUnseen = 0;
    Object.values(appState.sessions).forEach((session) => {
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
  }, [appState.sessions]);

  const handleMessage = useCallback(
    (event: MessageEvent, subdomain: string) => {
      const data = JSON.parse(event.data as string);
      const { cmd } = data;

      if (!subdomain || subdomain === "") {
        console.warn(
          "Received WebSocket message with empty subdomain, ignoring",
        );
        return;
      }

      setAppState((prevState) => {
        const newSessions = { ...prevState.sessions };

        const existingSession = newSessions[subdomain];
        const session = existingSession || {
          url: `${subdomain}.${Utils.siteUrl}`,
          domain: Utils.siteUrl,
          subdomain: subdomain,
          httpRequests: [],
          dnsRequests: [],
          timestamp: null,
          requests: {},
          visited: {},
          selectedRequest: null,
          token: "",
          dnsRecords: [],
        };

        if (cmd === "invalid_token") {
          const token = data["token"];
          if (token) {
            try {
              const tokenSubdomain = JSON.parse(atob(token.split(".")[1]))[
                "subdomain"
              ];
              toast.error(
                `Invalid token for ${tokenSubdomain}, request a new URL`,
                Utils.toastOptions,
              );
            } catch (err) {
              console.error("Error parsing token:", err);
            }
          }
        } else if (cmd === "requests") {
          const requests = data["data"].map(JSON.parse);
          requests.forEach((request: Request) => {
            const key = request["_id"];

            if (!session.requests[key]) {
              session.requests[key] = request;

              if (request["type"] === "http") {
                if (!session.httpRequests.some((r) => r["_id"] === key)) {
                  session.httpRequests.push(request);
                }
              } else if (request["type"] === "dns") {
                if (!session.dnsRequests.some((r) => r["_id"] === key)) {
                  session.dnsRequests.push(request);
                }
              }
            }
          });
        } else if (cmd === "request") {
          const request = JSON.parse(data["data"]);
          const key = request["_id"];

          if (!session.requests[key]) {
            request["new"] = true;
            session.requests[key] = request;

            const isFirstRequest =
              session.httpRequests.length === 0 &&
              session.dnsRequests.length === 0;

            if (request["type"] === "http") {
              if (!session.httpRequests.some((r) => r["_id"] === key)) {
                session.httpRequests.push(request);
              }
            } else if (request["type"] === "dns") {
              if (!session.dnsRequests.some((r) => r["_id"] === key)) {
                session.dnsRequests.push(request);
              }
            }

            if (isFirstRequest && sharedRequest) {
              setTimeout(() => {
                toast.info(
                  `New request received on your subdomain. Click it in the sidebar to view.`,
                  Utils.toastOptions,
                );
              }, 100);
            }

            updateDocumentTitle();
          }
        } else if (cmd === "dns_records") {
          session.dnsRecords = data.records;
        }

        if (subdomain && subdomain !== "") {
          newSessions[subdomain] = session;
        }

        return { ...prevState, sessions: newSessions };
      });
    },
    [sharedRequest, updateDocumentTitle],
  );

  const protocol = document.location.protocol === "https:" ? "wss" : "ws";
  const ws_url = `${protocol}://${document.location.host}/api/ws2`;

  const onOpen = useCallback(() => {
    setAppState((prevState) => {
      const newSessions: Record<string, AppSession> = {};
      Object.keys(prevState.sessions).forEach((subdomain) => {
        const existingSession = prevState.sessions[subdomain];
        newSessions[subdomain] = {
          ...existingSession,
          httpRequests: existingSession.httpRequests || [],
          dnsRequests: existingSession.dnsRequests || [],
          requests: existingSession.requests || {},
          visited: existingSession.visited || {},
        };
      });
      return { ...prevState, sessions: newSessions };
    });
  }, []);

  useWebSocket({
    ws_url,
    onUpdate: handleMessage,
    onOpen,
    sessions: appState.sessions,
    websocketRef,
  });

  useEffect(() => {
    const initializeSessions = async () => {
      const allSessions = Utils.getAllSessions();
      if (allSessions.length === 0) {
        try {
          const { subdomain, token } = await Utils.getRandomSubdomain();

          const newSession = {
            subdomain,
            token,
            createdAt: new Date().toISOString(),
            unseenRequests: 0,
          };

          localStorage.setItem("sessions", JSON.stringify([newSession]));
          localStorage.setItem("selectedSessionIndex", "0");

          setAppState((prevState) => ({
            ...prevState,
            sessions: {
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
            },
            activeSession: subdomain,
          }));
        } catch (error) {
          console.error("Failed to create initial session:", error);
          toast.error("Failed to create initial session", Utils.toastOptions);
        }
      } else {
        const sessions: Record<string, AppSession> = {};
        allSessions.forEach((session) => {
          sessions[session.subdomain] = {
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

        const finalActiveSession = sessions[activeSessionSubdomain]
          ? activeSessionSubdomain
          : allSessions[0].subdomain;

        setAppState((prevState) => ({
          ...prevState,
          sessions,
          activeSession: finalActiveSession,
        }));
      }
    };

    initializeSessions();
  }, []);

  useEffect(() => {
    const checkSharedRequest = async () => {
      const urlParams = new URLSearchParams(window.location.search);
      const requestId = urlParams.get("request");
      const sessionId = urlParams.get("session");

      if (requestId && sessionId) {
        setSharedRequest(requestId);

        try {
          const existingSessions = Utils.getAllSessions();
          const sessionExists = existingSessions.some(
            (s) => s.subdomain === sessionId,
          );

          if (!sessionExists) {
            try {
              const response = await fetch(
                `/api/get_token?subdomain=${sessionId}`,
              );
              const data = await response.json();

              if (data.token) {
                const newSession = {
                  subdomain: sessionId,
                  token: data.token,
                  createdAt: new Date().toISOString(),
                  unseenRequests: 0,
                };

                existingSessions.push(newSession);
                localStorage.setItem(
                  "sessions",
                  JSON.stringify(existingSessions),
                );

                if (websocketRef.current?.readyState === WebSocket.OPEN) {
                  const sessionTokens = [
                    { token: data.token, subdomain: sessionId },
                  ];
                  websocketRef.current.send(
                    JSON.stringify({
                      cmd: "register_sessions",
                      sessions: sessionTokens,
                    }),
                  );
                }

                setAppState((prevState) => {
                  const newSessions = {
                    ...prevState.sessions,
                    [sessionId]: {
                      url: `${sessionId}.${Utils.siteUrl}`,
                      domain: Utils.siteUrl,
                      subdomain: sessionId,
                      httpRequests: [],
                      dnsRequests: [],
                      timestamp: null,
                      requests: {},
                      visited: {},
                      selectedRequest: requestId,
                      token: data.token,
                    },
                  };

                  return {
                    ...prevState,
                    sessions: newSessions,
                    activeSession: sessionId,
                  };
                });
              }
            } catch (error) {
              console.error("Failed to get token for shared request:", error);
            }
          } else {
            setAppState((prevState) => ({
              ...prevState,
              activeSession: sessionId,
            }));
          }

          navigate(location.pathname, { replace: true });
        } catch (error) {
          console.error("Error processing shared request:", error);
        }
      }
    };

    checkSharedRequest();
  }, [location, navigate]);

  const [themeState, setThemeState] = useState(Utils.getTheme());
  useEffect(() => {
    const handleThemeChange = () => {
      setThemeState(Utils.getTheme());
    };
    window.addEventListener("themeChange", handleThemeChange);
    return () => window.removeEventListener("themeChange", handleThemeChange);
  }, []);

  const markAllAsVisited = useCallback(() => {
    if (!appState.activeSession) return;

    setAppState((prevState) => {
      const activeSession = prevState.sessions[prevState.activeSession];
      if (!activeSession) return prevState;

      const newVisited = { ...activeSession.visited };
      Object.keys(activeSession.requests).forEach((key) => {
        newVisited[key] = true;
        activeSession.requests[key].new = false;
      });

      const newSessions = { ...prevState.sessions };
      newSessions[prevState.activeSession] = {
        ...activeSession,
        visited: newVisited,
      };

      return { ...prevState, sessions: newSessions };
    });

    updateDocumentTitle();
  }, [appState.activeSession, updateDocumentTitle]);

  const clickRequestAction = useCallback(
    (action: string, id: string) => {
      if (!appState.activeSession) return;

      setAppState((prevState) => {
        const activeSession = prevState.sessions[prevState.activeSession];
        if (!activeSession) return prevState;

        if (action === "select") {
          const newVisited = { ...activeSession.visited };
          newVisited[id] = true;
          if (activeSession.requests[id]) {
            activeSession.requests[id].new = false;
          }

          const newSessions = { ...prevState.sessions };
          newSessions[prevState.activeSession] = {
            ...activeSession,
            visited: newVisited,
            selectedRequest: id,
          };

          updateDocumentTitle();
          return { ...prevState, sessions: newSessions };
        } else if (action === "delete") {
          const newRequests = { ...activeSession.requests };
          delete newRequests[id];

          const combinedRequests = [
            ...activeSession.httpRequests,
            ...activeSession.dnsRequests,
          ];
          const deleteIndex = combinedRequests.findIndex((r) => r._id === id);
          const nextSelectedIndex = Math.min(
            deleteIndex + 1,
            combinedRequests.length - 1,
          );
          const nextSelectedIndex2 = Math.max(0, nextSelectedIndex);
          const nextSelectedId =
            nextSelectedIndex2 >= 0 &&
            nextSelectedIndex2 < combinedRequests.length
              ? combinedRequests[nextSelectedIndex2]._id
              : null;

          const newHttpRequests = activeSession.httpRequests.filter(
            (r) => r._id !== id,
          );
          const newDnsRequests = activeSession.dnsRequests.filter(
            (r) => r._id !== id,
          );

          const newSessions = { ...prevState.sessions };
          newSessions[prevState.activeSession] = {
            ...activeSession,
            requests: newRequests,
            httpRequests: newHttpRequests,
            dnsRequests: newDnsRequests,
            selectedRequest: nextSelectedId,
          };

          Utils.deleteRequest(id, activeSession.subdomain);

          return { ...prevState, sessions: newSessions };
        }

        return prevState;
      });
    },
    [appState.activeSession, updateDocumentTitle],
  );

  const updateSearchValue = useCallback((value: string) => {
    setAppState((prevState) => ({ ...prevState, searchValue: value }));
  }, []);

  const copyUrl = useCallback(() => {
    if (urlArea.current) {
      urlArea.current.select();
      document.execCommand("copy");
      toast.info("URL copied to clipboard", Utils.toastOptions);
    }
  }, []);

  const copyDomain = useCallback(() => {
    if (urlArea.current) {
      urlArea.current.select();
      document.execCommand("copy");
      toast.info("Domain copied to clipboard", Utils.toastOptions);
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

      setAppState((prevState) => {
        const newSessions = allSessions.reduce(
          (acc, session) => {
            acc[session.subdomain] = prevState.sessions[session.subdomain] || {
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
            return acc;
          },
          {} as Record<string, AppSession>,
        );

        return {
          ...prevState,
          sessions: newSessions,
          activeSession: subdomain,
        };
      });

      if (websocketRef.current?.readyState === WebSocket.OPEN) {
        websocketRef.current.send(
          JSON.stringify({
            cmd: "register_sessions",
            sessions: [{ token, subdomain }],
          }),
        );
      }

      toast.success(`Created new URL: ${subdomain}`, Utils.toastOptions);
    } catch (error) {
      console.error("Failed to create new URL:", error);
      toast.error("Failed to create new URL", Utils.toastOptions);
    }
  }, []);

  const deleteAllRequests = useCallback(() => {
    if (!appState.activeSession) return;

    setAppState((prevState) => {
      const activeSession = prevState.sessions[prevState.activeSession];
      if (!activeSession) return prevState;

      const newSessions = { ...prevState.sessions };
      newSessions[appState.activeSession] = {
        ...activeSession,
        httpRequests: [],
        dnsRequests: [],
        requests: {},
        selectedRequest: null,
      };

      return { ...prevState, sessions: newSessions };
    });

    Utils.deleteAll(appState.activeSession);
  }, [appState.activeSession]);

  const onToggleMenu = useCallback(() => {
    setAppState((prevState) => ({
      ...prevState,
      staticMenuInactive: !prevState.staticMenuInactive,
    }));
  }, []);

  const activeSessionData = appState.activeSession
    ? appState.sessions[appState.activeSession]
    : null;

  return (
    <div className={`layout-wrapper ${themeState}`}>
      <AppTopbar
        onToggleMenu={onToggleMenu}
        staticMenuInactive={appState.staticMenuInactive}
        sessions={appState.sessions}
        activeSession={appState.activeSession}
        copyUrl={copyUrl}
        copyDomain={copyDomain}
        handleNewURL={handleNewURL}
      />

      <div className="layout-main">
        <div className="layout-content">
          <AppSidebar
            user={activeSessionData}
            activeSession={appState.activeSession}
            sessions={appState.sessions}
            searchValue={appState.searchValue}
            deleteAllRequests={deleteAllRequests}
            markAllAsVisited={markAllAsVisited}
            clickRequestAction={clickRequestAction}
          />

          <div className="layout-main-content">
            <div className="grid">
              <div className="col-12">
                <Toolbar
                  className="mb-4"
                  left={
                    <div className="p-inputgroup">
                      <InputText
                        placeholder="Search"
                        value={appState.searchValue}
                        onChange={(e) => updateSearchValue(e.target.value)}
                      />
                      <Button
                        icon="pi pi-search"
                        className="p-button-primary"
                      />
                    </div>
                  }
                />
              </div>
            </div>

            <Routes>
              <Route
                path="/"
                element={
                  <RequestsPage
                    user={activeSessionData as any}
                    toast={toast}
                    activeSession={appState.activeSession}
                  />
                }
              />
              <Route
                path="/edit-response"
                element={
                  <EditResponsePage
                    user={activeSessionData as any}
                    toast={toast}
                    activeSession={appState.activeSession}
                  />
                }
              />
              <Route
                path="/dns-settings"
                element={
                  <DnsSettingsPage
                    user={activeSessionData as any}
                    toast={toast}
                    activeSession={activeSessionData}
                    dnsRecords={appState.dnsRecords}
                  />
                }
              />
            </Routes>
          </div>
        </div>
      </div>

      <ToastContainer />
    </div>
  );
};

export default App;

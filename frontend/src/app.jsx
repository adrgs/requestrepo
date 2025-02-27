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
import { ToastContainer, toast } from "react-toastify";
import "primereact/resources/themes/lara-light-blue/theme.css";
import "primereact/resources/primereact.min.css";
import "primeicons/primeicons.css";
import "primeflex/primeflex.css";
import "react-toastify/dist/ReactToastify.css";
import "./app.scss";

function useWebSocket(ws_url, onUpdate, onOpen, sessions, websocketRef) {
  const reconnectTimeoutRef = useRef(null);
  const isConnectingRef = useRef(false);
  const sessionsRef = useRef(sessions);
  const onOpenRef = useRef(onOpen);
  const heartbeatIntervalRef = useRef(null);
  const lastPongTimeRef = useRef(Date.now());

  // Update refs when dependencies change
  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  useEffect(() => {
    onOpenRef.current = onOpen;
  }, [onOpen]);

  // Function to send heartbeat ping
  const sendHeartbeat = useCallback(() => {
    if (websocketRef.current?.readyState === WebSocket.OPEN) {
      try {
        // Send a ping message
        websocketRef.current.send(JSON.stringify({ cmd: "ping" }));
        
        // Check if we haven't received a response for too long (10 seconds)
        const timeSinceLastPong = Date.now() - lastPongTimeRef.current;
        if (timeSinceLastPong > 10000) {
          console.warn("WebSocket connection may be stale, reconnecting...");
          // Force close and reconnect
          websocketRef.current.close();
        }
      } catch (error) {
        console.error("Error sending heartbeat:", error);
        // Connection is probably dead, force reconnect
        if (websocketRef.current) {
          websocketRef.current.close();
        }
      }
    }
  }, []);

  const connectWebSocket = useCallback(() => {
    // Prevent multiple connection attempts
    if (
      isConnectingRef.current ||
      websocketRef.current?.readyState === WebSocket.CONNECTING
    ) {
      return;
    }

    // Don't reconnect if already connected
    if (websocketRef.current?.readyState === WebSocket.OPEN) {
      return;
    }

    isConnectingRef.current = true;

    // Close existing connection before creating new one
    if (websocketRef.current) {
      try {
        websocketRef.current.close();
      } catch (err) {
        console.error("Error closing websocket:", err);
      }
    }

    // Clear any existing heartbeat interval
    if (heartbeatIntervalRef.current) {
      clearInterval(heartbeatIntervalRef.current);
      heartbeatIntervalRef.current = null;
    }

    const socket = new WebSocket(ws_url);
    websocketRef.current = socket;

    socket.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data);
        
        // Handle pong response
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
        // Send all valid session tokens on connect
        const sessionTokens = Object.entries(sessionsRef.current)
          .filter(([_, session]) => session && session.token)
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

        // Start the heartbeat
        heartbeatIntervalRef.current = setInterval(sendHeartbeat, 30000); // 30 seconds

        if (onOpenRef.current) {
          onOpenRef.current();
        }
      } catch (err) {
        console.error("Error in websocket onopen:", err);
      }
    };

    socket.onclose = (event) => {
      isConnectingRef.current = false;
      
      // Clear heartbeat interval on close
      if (heartbeatIntervalRef.current) {
        clearInterval(heartbeatIntervalRef.current);
        heartbeatIntervalRef.current = null;
      }
      
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }

      // Only attempt reconnect if this is still the current socket and it wasn't closed intentionally
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

  // Main WebSocket connection effect
  useEffect(() => {
    connectWebSocket();

    // Handle visibility change to reconnect when tab becomes visible again
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        // Check WebSocket state when tab becomes visible
        if (!websocketRef.current || 
            websocketRef.current.readyState === WebSocket.CLOSED || 
            websocketRef.current.readyState === WebSocket.CLOSING) {
          // Reset connecting flag in case it got stuck
          isConnectingRef.current = false;
          // Try to reconnect
          connectWebSocket();
        } else if (websocketRef.current.readyState === WebSocket.OPEN) {
          // Connection appears to be open, send a ping to verify
          sendHeartbeat();
        }
      }
    };

    // Add visibility change listener
    document.addEventListener('visibilitychange', handleVisibilityChange);

    return () => {
      // Clean up
      document.removeEventListener('visibilitychange', handleVisibilityChange);
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

const App = () => {
  const urlArea = useRef(null);
  const websocketRef = useRef(null);
  const location = useLocation();
  const navigate = useNavigate();
  const [appState, setAppState] = useState({
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

  // Move initial session check into useEffect
  useEffect(() => {
    const checkInitialSession = async () => {
      // Check for shared token in URL
      const urlParams = new URLSearchParams(window.location.search);
      const sharedToken = urlParams.get('share');
      
      if (sharedToken) {
        try {
          // Decode the token to get the subdomain
          const tokenParts = sharedToken.split('.');
          if (tokenParts.length >= 2) {
            let subdomain;
            try {
              const payload = JSON.parse(atob(tokenParts[1]));
              subdomain = payload.subdomain;
            } catch (e) {
              console.error("Failed to parse token payload:", e);
              throw new Error("Invalid token format");
            }
            
            if (!subdomain) {
              throw new Error("Token missing subdomain information");
            }
            
            // Get current sessions from localStorage
            const existingSessions = Utils.getAllSessions();
            
            // Check if this session already exists
            const sessionExists = existingSessions.some(session => 
              session.token === sharedToken || session.subdomain === subdomain
            );
            
            if (!sessionExists) {
              // Create new session object
              const newSession = {
                subdomain,
                token: sharedToken,
                createdAt: new Date().toISOString(),
                unseenRequests: 0,
              };
              
              // Add to existing sessions
              existingSessions.push(newSession);
              
              // Save updated sessions
              localStorage.setItem("sessions", JSON.stringify(existingSessions));
              
              // Store active session in sessionStorage to make it tab-specific
              sessionStorage.setItem("activeSessionSubdomain", subdomain);
              
              // Update app state
              setAppState(prevState => {
                const newSessions = {
                  ...prevState.sessions,
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
                    token: sharedToken,
                  }
                };
                
                return {
                  ...prevState,
                  sessions: newSessions,
                  activeSession: subdomain,
                };
              });
              
              // Send token to WebSocket if connected
              if (websocketRef.current?.readyState === WebSocket.OPEN) {
                websocketRef.current.send(
                  JSON.stringify({
                    cmd: "register_sessions",
                    sessions: [{ token: sharedToken, subdomain }],
                  })
                );
              }
              
              toast.success(`Added shared session: ${subdomain}`);
            } else {
              toast.info(`Session ${subdomain} already exists`);
              // Still make this the active session in this tab
              sessionStorage.setItem("activeSessionSubdomain", subdomain);
            }
            
            // Remove share parameter from URL without reloading
            navigate(location.pathname, { replace: true });
            
            // Remove share parameter from URL without reloading
            // Create a clean URL without any query parameters
            navigate(location.pathname, { 
              replace: true,
              search: '' // This explicitly removes all query parameters
            });

            // Remove share parameter from URL without triggering re-renders
            window.history.replaceState({}, document.title, window.location.pathname);
          } else {
            throw new Error("Invalid token format");
          }
        } catch (error) {
          console.error("Failed to process shared token:", error);
          toast.error(`Invalid shared token: ${error.message}`);
          
          // Remove share parameter from URL even if processing failed
          navigate(location.pathname, { replace: true });
          
          // Remove share parameter from URL even if processing failed
          navigate(location.pathname, { 
            replace: true, 
            search: '' // Clear all query parameters
          });

          // Remove share parameter from URL even if processing failed
          window.history.replaceState({}, document.title, window.location.pathname);
        }
      }

      // Separate logic for first-time users
      const createInitialSessionIfNeeded = async () => {
        if (!Utils.userHasSubdomain()) {
          // Only create a new session if none exist
          const existingSessions = Utils.getAllSessions();
          if (existingSessions.length === 0) {
            try {
              const { subdomain, token } = await Utils.getRandomSubdomain();

              // Create initial session array
              const newSession = {
                subdomain,
                token,
                createdAt: new Date().toISOString(),
                unseenRequests: 0,
              };

              // Save to localStorage
              localStorage.setItem("sessions", JSON.stringify([newSession]));
              localStorage.setItem("selectedSessionIndex", "0");

              // Update state
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
              toast.error("Failed to create initial session");
            }
          }
        }
      };

      // Only run the initial session creation if we didn't process a share token
      if (!sharedToken) {
        await createInitialSessionIfNeeded();
      }
    };

    checkInitialSession();
  }, []); // Include both location and navigate as dependencies

  const [themeState, setThemeState] = useState(Utils.getTheme());
  useEffect(() => {
    const handleThemeChange = () => {
      setThemeState(Utils.getTheme());
    };
    window.addEventListener("themeChange", handleThemeChange);
    return () => window.removeEventListener("themeChange", handleThemeChange);
  }, []);
  // Moved to a custom hook
  const handleMessage = useCallback((event, subdomain) => {
    const data = JSON.parse(event.data);
    const { cmd } = data;
    handleWebSocketData(cmd, data, subdomain);
  }, []);
  // Function to handle WebSocket data separately
  const handleWebSocketData = (cmd, data, subdomain) => {
    setAppState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const session = newSessions[subdomain] || {
        url: `${subdomain}.${Utils.siteUrl}`,
        domain: Utils.siteUrl,
        subdomain: subdomain,
        httpRequests: [],
        dnsRequests: [],
        requests: {},
        visited: {},
        selectedRequest: null,
        dnsRecords: [],
      };

      if (cmd === "invalid_token") {
        const token = data["token"];
        const subdomain = JSON.parse(atob(token.split(".")[1]))["subdomain"];
        toast.error(`Invalid token for ${subdomain}, request a new URL`);
      }
      if (cmd === "requests") {
        const requests = data["data"].map(JSON.parse);
        requests.forEach((request) => {
          const key = request["_id"];
          session.requests[key] = request;
          if (request["type"] === "http") {
            // Prevent duplicate requests
            if (!session.httpRequests.find((r) => r["_id"] === key)) {
              session.httpRequests.push(request);
            }
          } else if (request["type"] === "dns") {
            if (!session.dnsRequests.find((r) => r["_id"] === key)) {
              session.dnsRequests.push(request);
            }
          }
        });
      } else if (cmd === "request") {
        const request = JSON.parse(data["data"]);
        const key = request["_id"];
        request["new"] = true;
        session.requests[key] = request;
        if (request["type"] === "http") {
          session.httpRequests.push(request);
        } else if (request["type"] === "dns") {
          session.dnsRequests.push(request);
        }
      } else if (cmd === "dns_records") {
        session.dnsRecords = data.records;
      }

      newSessions[subdomain] = session;
      return { ...prevState, sessions: newSessions };
    });
  };

  // URL for WebSocket based on protocol
  const protocol = document.location.protocol === "https:" ? "wss" : "ws";
  const ws_url = `${protocol}://${document.location.host}/api/ws2`;

  const onOpen = () => {
    setAppState((prevState) => {
      const newSessions = {};
      Object.keys(prevState.sessions).forEach((subdomain) => {
        // Preserve existing session data
        const existingSession = prevState.sessions[subdomain];
        newSessions[subdomain] = {
          ...existingSession,
          // Only reset these if they're empty
          httpRequests: existingSession.httpRequests || [],
          dnsRequests: existingSession.dnsRequests || [],
          requests: existingSession.requests || {},
          visited: existingSession.visited || {},
        };
      });
      return { ...prevState, sessions: newSessions };
    });
  };

  // Use custom WebSocket hook
  useWebSocket(ws_url, handleMessage, onOpen, appState.sessions, websocketRef);

  // Initialize sessions in useEffect
  useEffect(() => {
    const initializeSessions = async () => {
      const allSessions = Utils.getAllSessions();
      if (allSessions.length === 0) {
        try {
          // Get new subdomain and token
          const { subdomain, token } = await Utils.getRandomSubdomain();

          // Get current sessions array (should be empty but let's be consistent)
          const sessionsStr = localStorage.getItem("sessions");
          const sessions = JSON.parse(sessionsStr || "[]");

          // Create the new session
          const newSession = {
            subdomain,
            token,
            createdAt: new Date().toISOString(),
            unseenRequests: 0,
          };

          // Add new session to array
          sessions.push(newSession);

          // Update localStorage
          localStorage.setItem("sessions", JSON.stringify(sessions));
          
          // Save active session to sessionStorage (tab-specific)
          sessionStorage.setItem("activeSessionSubdomain", subdomain);

          // Update parent component's state via onSessionChange pattern
          setAppState((prev) => ({
            ...prev,
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
          console.error("Error creating default session:", error);
          toast.error("Failed to create initial session");
        }
      } else {
        // Convert array to object format
        const sessions = allSessions.reduce(
          (acc, session) => ({
            ...acc,
            [session.subdomain]: {
              url: `${session.subdomain}.${Utils.siteUrl}`,
              domain: Utils.siteUrl,
              subdomain: session.subdomain,
              httpRequests: [],
              dnsRequests: [],
              timestamp: null,
              requests: {},
              visited: JSON.parse(
                localStorage.getItem(`visited_${session.subdomain}`) || "{}",
              ),
              selectedRequest: localStorage.getItem(
                `lastSelectedRequest_${session.subdomain}`,
              ),
              token: session.token,
            },
          }),
          {},
        );

        // Get the active session from sessionStorage (tab-specific) or fall back to first session
        const activeSessionSubdomain = sessionStorage.getItem("activeSessionSubdomain") || 
          allSessions[0]?.subdomain || "";

        // Validate that the active session actually exists in our sessions
        const finalActiveSession = sessions[activeSessionSubdomain] ? 
          activeSessionSubdomain : allSessions[0]?.subdomain || "";
        
        if (finalActiveSession && finalActiveSession !== activeSessionSubdomain) {
          // Update sessionStorage if we had to fall back to a different session
          sessionStorage.setItem("activeSessionSubdomain", finalActiveSession);
        }

        setAppState((prev) => ({
          ...prev,
          sessions,
          activeSession: finalActiveSession,
        }));
      }
    };

    initializeSessions();
  }, []); // Run once on mount

  useEffect(() => {
    const text = `Dashboard - ${Utils.siteUrl}`;
    const totalUnseen = Object.values(appState.sessions).reduce((sum, session) => {
      const unseenCount =
        session.httpRequests.length +
        session.dnsRequests.length -
        Object.keys(session.visited || {}).length;
      return sum + Math.max(0, unseenCount);
    }, 0);

    document.title = totalUnseen <= 0 ? text : `(${totalUnseen}) ${text}`;
  }, [appState.sessions]);

  useEffect(() => {
    Utils.initTheme();
    const handleStorageChange = (e) => {
      // Ignore null events
      if (!e.key) return;

      if (e.key === "sessions" || e.key === "selectedSessionIndex") {
        // Update state based on new sessions data
        setAppState((prevState) => {
          try {
            const sessions = JSON.parse(e.newValue || "[]");
            const selectedIndex = parseInt(
              localStorage.getItem("selectedSessionIndex") || "0",
            );
            const validIndex = Math.max(
              0,
              Math.min(selectedIndex, sessions.length - 1),
            );

            // Send update_tokens to WebSocket
            if (websocketRef.current?.readyState === WebSocket.OPEN) {
              websocketRef.current.send(
                JSON.stringify({
                  cmd: "update_tokens",
                  tokens: sessions.map((s) => s.token),
                }),
              );
            }

            // Convert sessions array to our sessions state format while preserving existing state
            const newSessions = sessions.reduce(
              (acc, session) => ({
                ...acc,
                [session.subdomain]: {
                  ...(prevState.sessions[session.subdomain] || {}), // Preserve existing state if available
                  url: `${session.subdomain}.${Utils.siteUrl}`,
                  domain: Utils.siteUrl,
                  subdomain: session.subdomain,
                  httpRequests:
                    prevState.sessions[session.subdomain]?.httpRequests || [],
                  dnsRequests:
                    prevState.sessions[session.subdomain]?.dnsRequests || [],
                  timestamp:
                    prevState.sessions[session.subdomain]?.timestamp || null,
                  requests:
                    prevState.sessions[session.subdomain]?.requests || {},
                  visited:
                    prevState.sessions[session.subdomain]?.visited ||
                    JSON.parse(
                      localStorage.getItem(`visited_${session.subdomain}`) ||
                        "{}",
                    ),
                  selectedRequest:
                    prevState.sessions[session.subdomain]?.selectedRequest ||
                    localStorage.getItem(
                      `lastSelectedRequest_${session.subdomain}`,
                    ),
                  token: session.token,
                },
              }),
              {},
            );

            return {
              ...prevState,
              sessions: newSessions,
              activeSession: sessions[validIndex]?.subdomain || "",
            };
          } catch (error) {
            console.error("Error parsing sessions data:", error);
            return prevState;
          }
        });
      } else if (e.key.startsWith("deleteAll_")) {
        const targetSubdomain = e.key.replace("deleteAll_", "");
        setAppState((prevState) => {
          const newSessions = { ...prevState.sessions };
          if (newSessions[targetSubdomain]) {
            newSessions[targetSubdomain] = {
              ...newSessions[targetSubdomain],
              url: `${targetSubdomain}.${Utils.siteUrl}`,
              domain: Utils.siteUrl,
              subdomain: targetSubdomain,
              httpRequests: [],
              dnsRequests: [],
              requests: {},
              visited: {},
              selectedRequest: null,
              timestamp: null,
            };
          }
          return { ...prevState, sessions: newSessions };
        });
      } else if (e.key.startsWith("visited_") || e.key.startsWith("token_")) {
        if (e.key.startsWith("visited_")) {
          const targetSubdomain = e.key.replace("visited_", "");
          setAppState((prevState) => {
            const newSessions = { ...prevState.sessions };
            if (newSessions[targetSubdomain]) {
              try {
                const newVisited = JSON.parse(e.newValue || "{}");
                newSessions[targetSubdomain] = {
                  ...newSessions[targetSubdomain],
                  visited: newVisited,
                  requests: Object.fromEntries(
                    Object.entries(newSessions[targetSubdomain].requests).map(
                      ([key, request]) => [
                        key,
                        newVisited[key] ? { ...request, new: false } : request,
                      ],
                    ),
                  ),
                };
              } catch (err) {
                console.error("Error parsing visited data:", err);
              }
            }
            return { ...prevState, sessions: newSessions };
          });
        } else if (e.key.startsWith("token_")) {
          // Only reload if this is a manual token change, not our automatic token refresh
          if (!e.newValue || e.newValue === "") {
            document.location.reload();
          }
        }
      } else if (e.key?.startsWith("lastSelectedRequest_")) {
        const targetSubdomain = e.key.replace("lastSelectedRequest_", "");
        setAppState((prevState) => {
          const newSessions = { ...prevState.sessions };
          if (
            newSessions[targetSubdomain] &&
            newSessions[targetSubdomain].requests[e.newValue]
          ) {
            newSessions[targetSubdomain].selectedRequest = e.newValue;
            newSessions[targetSubdomain].requests[e.newValue].new = false;
          }
          return { ...prevState, sessions: newSessions };
        });
      } else if (e.key?.startsWith("lastDeletedRequest_")) {
        const targetSubdomain = e.key.replace("lastDeletedRequest_", "");
        const requestId = e.newValue;
        setAppState((prevState) => {
          const newSessions = { ...prevState.sessions };
          if (newSessions[targetSubdomain]) {
            delete newSessions[targetSubdomain].requests[requestId];
            delete newSessions[targetSubdomain].visited[requestId];
            newSessions[targetSubdomain].httpRequests = newSessions[
              targetSubdomain
            ].httpRequests.filter((value) => value["_id"] !== requestId);
            newSessions[targetSubdomain].dnsRequests = newSessions[
              targetSubdomain
            ].dnsRequests.filter((value) => value["_id"] !== requestId);
          }
          return { ...prevState, sessions: newSessions };
        });
      }
    };

    window.addEventListener("storage", handleStorageChange);
    return () => {
      window.removeEventListener("storage", handleStorageChange);
    };
  }, [appState.activeSession]); // Add dependency on activeSession

  const markAllAsVisited = () => {
    setAppState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const activeSession = newSessions[appState.activeSession];

      if (activeSession) {
        const updatedRequests = {};
        const visited = {};

        Object.entries(activeSession.requests).forEach(([key, value]) => {
          updatedRequests[key] = { ...value, new: false };
          visited[key] = true;
        });

        activeSession.requests = updatedRequests;
        activeSession.visited = visited;

        localStorage.setItem(
          `visited_${appState.activeSession}`,
          JSON.stringify(visited),
        );
      }

      return { ...prevState, sessions: newSessions };
    });
  };

  const clickRequestAction = (action, id) => {
    setAppState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const activeSession = newSessions[appState.activeSession];

      if (!activeSession) return prevState;

      if (action === "select") {
        if (activeSession.requests[id] !== undefined) {
          activeSession.selectedRequest = id;
          activeSession.requests[id]["new"] = false;
          if (activeSession.visited[id] === undefined) {
            activeSession.visited[id] = true;
            localStorage.setItem(
              `visited_${appState.activeSession}`,
              JSON.stringify(activeSession.visited),
            );
          }
          localStorage.setItem(
            `lastSelectedRequest_${appState.activeSession}`,
            id,
          );
        }
      } else if (action === "delete") {
        const combinedRequests = [
          ...activeSession.httpRequests,
          ...activeSession.dnsRequests,
        ];
        const deleteIndex = combinedRequests.findIndex(
          (request) => request["_id"] === id,
        );
        let nextSelectedIndex =
          deleteIndex >= combinedRequests.length - 1
            ? deleteIndex - 1
            : deleteIndex + 1;
        nextSelectedIndex = Math.max(
          0,
          Math.min(nextSelectedIndex, combinedRequests.length - 1),
        );
        const nextSelectedId =
          combinedRequests.length > 0
            ? combinedRequests[nextSelectedIndex]["_id"]
            : undefined;

        delete activeSession.requests[id];
        delete activeSession.visited[id];

        activeSession.httpRequests = activeSession.httpRequests.filter(
          (request) => request["_id"] !== id,
        );
        activeSession.dnsRequests = activeSession.dnsRequests.filter(
          (request) => request["_id"] !== id,
        );

        if (
          id ===
          localStorage.getItem(`lastSelectedRequest_${appState.activeSession}`)
        ) {
          localStorage.setItem(
            `lastSelectedRequest_${appState.activeSession}`,
            nextSelectedId,
          );
          activeSession.selectedRequest = nextSelectedId;
        }

        Utils.deleteRequest(id, appState.activeSession).then(() => {
          localStorage.setItem(
            `visited_${appState.activeSession}`,
            JSON.stringify(activeSession.visited),
          );
          localStorage.setItem(`lastDeletedRequest_${appState.activeSession}`, id);
        });
      } else if (action === "reset") {
        activeSession.selectedRequest = undefined;
        localStorage.setItem(
          `lastSelectedRequest_${appState.activeSession}`,
          undefined,
        );
      }

      return { ...prevState, sessions: newSessions };
    });
  };

  const updateSearchValue = (val) => {
    setAppState((prevState) => ({ ...prevState, searchValue: val }));
  };

  const copyUrl = () => {
    const fullUrl = `${window.location.protocol}//${urlArea.current.value}/`;
    if (!navigator.clipboard) {
      urlArea.current.select();
      document.execCommand("copy");
    } else {
      navigator.clipboard.writeText(fullUrl);
    }
    toast.info("URL copied to clipboard!", {
      position: "bottom-center",
      autoClose: 2500,
      hideProgressBar: false,
      closeOnClick: true,
      pauseOnHover: true,
      draggable: true,
      dark: Utils.isDarkTheme(),
    });
  };

  const copyDomain = () => {
    if (!navigator.clipboard) {
      urlArea.current.select();
      document.execCommand("copy");
    } else {
      navigator.clipboard.writeText(urlArea.current.value);
    }
    toast.info("Domain copied to clipboard!", {
      position: "bottom-center",
      autoClose: 2500,
      hideProgressBar: false,
      closeOnClick: true,
      pauseOnHover: true,
      draggable: true,
      dark: Utils.isDarkTheme(),
    });
  };

  const handleNewURL = async () => {
    try {
      const activeSession = appState.sessions[appState.activeSession];
      if (!activeSession) {
        throw new Error("No active session found");
      }

      // Get new subdomain and token
      const { subdomain, token } = await Utils.getRandomSubdomain();

      // Get current sessions array from localStorage
      const sessionsStr = localStorage.getItem("sessions");
      const sessions = JSON.parse(sessionsStr || "[]");

      // Update the session in localStorage
      const sessionIndex = sessions.findIndex(
        (s) => s.subdomain === activeSession.subdomain,
      );
      if (sessionIndex !== -1) {
        sessions[sessionIndex] = {
          ...sessions[sessionIndex],
          subdomain,
          token,
        };
        localStorage.setItem("sessions", JSON.stringify(sessions));
      }

      setAppState((prevState) => {
        const newSessions = Object.keys(prevState.sessions).reduce(
          (acc, key) => {
            if (key === activeSession.subdomain) {
              // Insert the new subdomain in place of the old one
              acc[subdomain] = {
                domain: Utils.siteUrl,
                url: `${subdomain}.${Utils.siteUrl}`,
                subdomain: subdomain,
                token: token,
                httpRequests: [],
                dnsRequests: [],
                timestamp: null,
                requests: {},
                visited: {},
                selectedRequest: null,
              };
            } else {
              // Keep other sessions as is
              acc[key] = prevState.sessions[key];
            }
            return acc;
          },
          {},
        );

        // Clean up old session data from localStorage
        localStorage.removeItem(`visited_${activeSession.subdomain}`);
        localStorage.removeItem(
          `lastSelectedRequest_${activeSession.subdomain}`,
        );
        localStorage.removeItem(`token_${activeSession.subdomain}`);

        // Send update_tokens to WebSocket
        if (websocketRef.current?.readyState === WebSocket.OPEN) {
          websocketRef.current.send(
            JSON.stringify({
              cmd: "update_tokens",
              tokens: sessions.map((s) => s.token),
            }),
          );
        }

        return {
          ...prevState,
          sessions: newSessions,
          activeSession: subdomain,
        };
      });
    } catch (error) {
      toast.error(`Failed to update session URL: ${error.message}`);
    }
  };

  const deleteAllRequests = () => {
    setAppState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const activeSession = newSessions[appState.activeSession];

      if (activeSession) {
        // Call the API to delete all requests
        Utils.deleteAll(appState.activeSession)
          .then(() => {
            // Update the current tab's state
            setAppState((prevState) => {
              const newSessions = { ...prevState.sessions };
              if (newSessions[appState.activeSession]) {
                // Preserve the session properties while clearing request data
                newSessions[appState.activeSession] = {
                  ...newSessions[appState.activeSession],
                  url: `${appState.activeSession}.${Utils.siteUrl}`,
                  domain: Utils.siteUrl,
                  subdomain: appState.activeSession,
                  httpRequests: [],
                  dnsRequests: [],
                  requests: {},
                  visited: {},
                  selectedRequest: null,
                  timestamp: null,
                };
              }

              // Notify other tabs about the deletion for this specific session
              localStorage.setItem(
                `deleteAll_${appState.activeSession}`,
                Date.now().toString(),
              );

              // Clean up the storage item immediately after setting it
              setTimeout(() => {
                localStorage.removeItem(`deleteAll_${appState.activeSession}`);
              }, 100);

              return {
                ...prevState,
                sessions: newSessions,
              };
            });
          })
          .catch((error) => {
            console.error("Failed to delete all requests:", error);
            toast.error("Failed to delete all requests");
          });
      }

      return {
        ...prevState,
        sessions: newSessions,
      };
    });
  };

  const onToggleMenu = (event) => {
    event.preventDefault();
    setAppState((prevState) => {
      const isDesktop = window.innerWidth > 768;
      if (isDesktop) {
        if (prevState.layoutMode === "overlay") {
          return {
            ...prevState,
            overlayMenuActive: !prevState.overlayMenuActive,
          };
        } else if (prevState.layoutMode === "static") {
          return {
            ...prevState,
            staticMenuInactive: !prevState.staticMenuInactive,
          };
        }
      } else {
        return { ...prevState, mobileMenuActive: !prevState.mobileMenuActive };
      }
    });
  };

  // Update Utils to use sessionStorage for active session
  useEffect(() => {
    // Update sessionStorage whenever active session changes
    if (appState.activeSession) {
      sessionStorage.setItem("activeSessionSubdomain", appState.activeSession);
    }
  }, [appState.activeSession]);

  // Component rendering logic
  return (
    <div className="layout-wrapper layout-static">
      <AppTopbar
        onToggleMenu={onToggleMenu}
        updateSearchValue={updateSearchValue}
        sessions={appState.sessions}
        activeSession={appState.activeSession}
        onSessionChange={(session) =>
          setAppState((prev) => {
            try {
              // Get the session data from localStorage
              const sessionsStr = localStorage.getItem("sessions");
              if (!sessionsStr) return prev;

              const sessions = JSON.parse(sessionsStr);
              const sessionData = sessions.find((s) => s.subdomain === session);
              if (!sessionData) return prev;

              // Preserve existing session data if it exists
              const existingSession = prev.sessions[session] || {
                url: `${sessionData.subdomain}.${Utils.siteUrl}`,
                domain: Utils.siteUrl,
                subdomain: sessionData.subdomain,
                httpRequests: [],
                dnsRequests: [],
                timestamp: null,
                requests: {},
                visited: {},
                selectedRequest: null,
                token: sessionData.token,
              };

              // Send update_tokens to WebSocket
              if (websocketRef.current?.readyState === WebSocket.OPEN) {
                websocketRef.current.send(
                  JSON.stringify({
                    cmd: "update_tokens",
                    tokens: sessions.map((s) => s.token),
                  }),
                );
              }

              // Update session storage with the new active session
              sessionStorage.setItem("activeSessionSubdomain", session);

              return {
                ...prev,
                sessions: {
                  ...prev.sessions,
                  [session]: existingSession,
                },
                activeSession: session,
              };
            } catch (error) {
              console.error("Error updating session state:", error);
              return prev;
            }
          })
        }
        onSessionRemove={(subdomain) => {
          setAppState((prev) => {
            try {
              const newSessions = { ...prev.sessions };

              // Don't allow removing the last session
              if (Object.keys(newSessions).length <= 1) {
                toast.warn("Cannot remove the last session");
                return prev;
              }

              delete newSessions[subdomain];

              // Determine new active session
              let newActiveSession = prev.activeSession;
              if (subdomain === prev.activeSession) {
                const remainingSessions = Object.keys(newSessions);
                newActiveSession = remainingSessions[0];
              }

              // Get updated tokens from localStorage
              const sessionsStr = localStorage.getItem("sessions");
              if (sessionsStr) {
                const sessions = JSON.parse(sessionsStr);
                // Send update_tokens to WebSocket
                if (websocketRef.current?.readyState === WebSocket.OPEN) {
                  websocketRef.current.send(
                    JSON.stringify({
                      cmd: "update_tokens",
                      tokens: sessions.map((s) => s.token),
                    }),
                  );
                }
              }

              return {
                ...prev,
                sessions: newSessions,
                activeSession: newActiveSession,
              };
            } catch (error) {
              console.error("Error removing session:", error);
              return prev;
            }
          });
        }}
      />

      <AppSidebar
        user={appState.sessions[appState.activeSession]}
        searchValue={appState.searchValue}
        clickRequestAction={clickRequestAction}
        deleteAllRequests={deleteAllRequests}
        markAllAsVisited={markAllAsVisited}
        activeSession={appState.activeSession}
      />

      <div className="layout-main">
        <div className="grid">
          <div className="col-12">
            <Toolbar
              style={{ lineHeight: "3", borderRadius: "5px 5px 0px 0px" }}
              left={
                <div style={{ textAlign: "center" }}>
                  <a href="#/requests">
                    <Button
                      label="Requests"
                      icon="pi pi-arrow-down"
                      className="p-button-text p-button-secondary"
                      style={{ marginRight: ".25em" }}
                    />
                  </a>
                  <a href="#/edit-response">
                    <Button
                      label="Response"
                      icon="pi pi-pencil"
                      className="p-button-text p-button-secondary"
                      style={{ marginRight: ".25em" }}
                    />
                  </a>
                  <a href="#/dns-settings">
                    <Button
                      label="DNS"
                      icon="pi pi-home"
                      className="p-button-text p-button-secondary"
                    />
                  </a>
                </div>
              }
              right={
                <div style={{ textAlign: "center" }}>
                  <InputText
                    type="text"
                    placeholder="Your URL"
                    value={
                      appState.sessions[appState.activeSession]
                        ? `${appState.sessions[appState.activeSession].subdomain}.${appState.sessions[appState.activeSession].domain}`
                        : ""
                    }
                    style={{ width: "300px", marginRight: "1em" }}
                    readOnly
                    ref={urlArea}
                    onClick={copyDomain}
                  />
                  <Button
                    label="Copy URL"
                    icon="pi pi-copy"
                    className="p-button-success"
                    style={{ marginRight: ".25em" }}
                    onClick={copyUrl}
                  />
                  <Button
                    label="New URL"
                    icon="pi pi-refresh"
                    onClick={handleNewURL}
                  />
                </div>
              }
            />
            <Routes>
              <Route
                exact
                path="/"
                element={
                  <RequestsPage user={appState.sessions[appState.activeSession]} />
                }
              />
              <Route
                path="/requests"
                element={
                  <RequestsPage user={appState.sessions[appState.activeSession]} />
                }
              />
              <Route
                path="/edit-response"
                element={
                  <EditResponsePage
                    content={appState.response.raw}
                    statusCode={appState.response.status_code}
                    headers={appState.response.headers}
                    user={appState.sessions[appState.activeSession]}
                    fetched={appState.response.fetched}
                    toast={toast}
                  />
                }
              />
              <Route
                path="/dns-settings"
                element={
                  <DnsSettingsPage
                    user={appState.sessions[appState.activeSession]}
                    dnsRecords={
                      appState.sessions[appState.activeSession]?.dnsRecords || []
                    }
                    toast={toast}
                    activeSession={appState.sessions[appState.activeSession]}
                  />
                }
              />
            </Routes>
          </div>
        </div>
      </div>

      <ToastContainer key={themeState} theme={themeState} />
      <div className="layout-mask"></div>
    </div>
  );
};

export default App;

import React, { useState, useEffect, useRef, useCallback } from "react";
import { Route, Routes } from "react-router-dom";
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

const genRanHex = (size) =>
  [...Array(size)]
    .map(() => Math.floor(Math.random() * 16).toString(16))
    .join("");
// Custom hook for WebSocket with reconnection logic
function useWebSocket(ws_url, onUpdate, onOpen, sessions, websocketRef) {
  const reconnectTimeoutRef = useRef(null);
  const isConnectingRef = useRef(false);
  const sessionsRef = useRef(sessions);
  const onOpenRef = useRef(onOpen);

  // Update refs when dependencies change
  useEffect(() => {
    sessionsRef.current = sessions;
  }, [sessions]);

  useEffect(() => {
    onOpenRef.current = onOpen;
  }, [onOpen]);

  useEffect(() => {
    const connectWebSocket = () => {
      // Prevent multiple connection attempts
      if (isConnectingRef.current || websocketRef.current?.readyState === WebSocket.CONNECTING) {
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
          console.error('Error closing websocket:', err);
        }
      }

      const socket = new WebSocket(ws_url);
      websocketRef.current = socket;

      socket.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          onUpdate(event, data.subdomain || Utils.subdomain);
        } catch (err) {
          console.error('Error handling websocket message:', err);
        }
      };

      socket.onopen = () => {
        isConnectingRef.current = false;
        
        try {
          // Send all valid session tokens on connect
          const sessionTokens = Object.entries(sessionsRef.current)
            .filter(([_, session]) => session && session.token)
            .map(([subdomain, session]) => ({
              token: session.token,
              subdomain: subdomain
            }));
          
          if (sessionTokens.length > 0) {
            socket.send(JSON.stringify({
              cmd: 'register_sessions',
              sessions: sessionTokens
            }));
          }

          if (onOpenRef.current) {
            onOpenRef.current();
          }
        } catch (err) {
          console.error('Error in websocket onopen:', err);
        }
      };

      socket.onclose = (event) => {
        isConnectingRef.current = false;
        if (reconnectTimeoutRef.current) {
          clearTimeout(reconnectTimeoutRef.current);
        }
        
        // Only attempt reconnect if this is still the current socket and it wasn't closed intentionally
        if (websocketRef.current === socket && event.code !== 1000) {
          reconnectTimeoutRef.current = setTimeout(connectWebSocket, 2500);
        }
      };

      socket.onerror = (error) => {
        console.error('WebSocket error:', error);
        isConnectingRef.current = false;
        if (websocketRef.current === socket) {
          socket.close();
        }
      };
    };

    connectWebSocket();

    return () => {
      isConnectingRef.current = false;
      if (websocketRef.current) {
        websocketRef.current.close(1000); // Normal closure
      }
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
    };
  }, [ws_url]); // Only reconnect if ws_url changes
}

const App = () => {
  const urlArea = useRef(null);
  const websocketRef = useRef(null);
  const [state, setState] = useState({
    layoutMode: "static",
    layoutColorMode: "light",
    staticMenuInactive: false,
    overlayMenuActive: false,
    mobileMenuActive: false,
    sessions: {},  // Initialize empty, then update in useEffect
    activeSession: '',
    searchValue: "",
    response: { raw: "", headers: [], status_code: 200, fetched: false },
    dnsRecords: [],
    dnsFetched: false
  });

  // Move initial session check into useEffect
  useEffect(() => {
    const checkInitialSession = async () => {
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
              unseenRequests: 0
            };
            
            // Save to localStorage
            localStorage.setItem('sessions', JSON.stringify([newSession]));
            localStorage.setItem('selectedSessionIndex', '0');

            // Update state
            setState(prevState => ({
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
                  token: token
                }
              },
              activeSession: subdomain
            }));
          } catch (error) {
            console.error('Failed to create initial session:', error);
            toast.error('Failed to create initial session');
          }
        }
      }
    };

    checkInitialSession();
  }, []); // Run once on mount

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
    console.log('WebSocket message received:', new Date().toISOString());
    const data = JSON.parse(event.data);
    const { cmd } = data;
    handleWebSocketData(cmd, data, subdomain);
  }, []);
  // Function to handle WebSocket data separately
  const handleWebSocketData = (cmd, data, subdomain) => {
    setState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const session = newSessions[subdomain] || {
        url: `${subdomain}.${Utils.siteUrl}`,
        domain: Utils.siteUrl,
        subdomain: subdomain,
        httpRequests: [],
        dnsRequests: [],
        requests: {},
        visited: {},
        selectedRequest: null
      };

      if (cmd === "requests") {
        const requests = data["data"].map(JSON.parse);
        requests.forEach((request) => {
          const key = request["_id"];
          session.requests[key] = request;
          if (request["type"] === "http") {
            // Prevent duplicate requests
            if (!session.httpRequests.find(r => r["_id"] === key)) {
              session.httpRequests.push(request);
            }
          } else if (request["type"] === "dns") {
            if (!session.dnsRequests.find(r => r["_id"] === key)) {
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
      }

      newSessions[subdomain] = session;
      return { ...prevState, sessions: newSessions };
    });
  };

  // URL for WebSocket based on protocol
  const protocol = document.location.protocol === "https:" ? "wss" : "ws";
  const ws_url = `${protocol}://${document.location.host}/api/ws`;

  const onOpen = () => {
    setState((prevState) => {
      const newSessions = {};
      Object.keys(prevState.sessions).forEach(subdomain => {
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
  useWebSocket(ws_url, handleMessage, onOpen, state.sessions, websocketRef);

  // Initialize sessions in useEffect
  useEffect(() => {
    const initializeSessions = async () => {
      const allSessions = Utils.getAllSessions();
      if (allSessions.length === 0) {
        try {
          // Get new subdomain and token
          const { subdomain, token } = await Utils.getRandomSubdomain();
          
          // Get current sessions array (should be empty but let's be consistent)
          const sessionsStr = localStorage.getItem('sessions');
          const sessions = JSON.parse(sessionsStr || '[]');
          
          // Create the new session
          const newSession = {
            subdomain,
            token,
            createdAt: new Date().toISOString(),
            unseenRequests: 0
          };
          
          // Add new session to array
          sessions.push(newSession);
          
          // Update localStorage
          localStorage.setItem('sessions', JSON.stringify(sessions));
          localStorage.setItem('selectedSessionIndex', '0');

          // Update parent component's state via onSessionChange pattern
          setState(prev => ({
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
                token: token
              }
            },
            activeSession: subdomain
          }));

        } catch (error) {
          console.error('Error creating default session:', error);
          toast.error('Failed to create initial session');
        }
      } else {
        // Get the selected session index
        const selectedIndex = parseInt(localStorage.getItem('selectedSessionIndex') || '0');
        const validIndex = Math.max(0, Math.min(selectedIndex, allSessions.length - 1));
        
        const sessions = allSessions.reduce((acc, session) => ({
          ...acc,
          [session.subdomain]: {
            url: `${session.subdomain}.${Utils.siteUrl}`,
            domain: Utils.siteUrl,
            subdomain: session.subdomain,
            httpRequests: [],
            dnsRequests: [],
            timestamp: null,
            requests: {},
            visited: JSON.parse(localStorage.getItem(`visited_${session.subdomain}`) || "{}"),
            selectedRequest: localStorage.getItem(`lastSelectedRequest_${session.subdomain}`),
            token: session.token
          }
        }), {});

        setState(prev => ({
          ...prev,
          sessions,
          activeSession: Utils.getActiveSession()?.subdomain || Utils.getAllSessions()[0]?.subdomain || ''
        }));
      }
    };

    initializeSessions();
  }, []); // Run once on mount

  useEffect(() => {
    const activeSession = state.sessions[state.activeSession];
    const text = `Dashboard - ${Utils.siteUrl}`;
    if (activeSession) {
      const n =
        activeSession.httpRequests.length +
        activeSession.dnsRequests.length -
        Object.keys(activeSession.visited || {}).length;
      document.title = n <= 0 ? text : `(${n}) ${text}`;
    } else {
      document.title = text;
    }
  }, [state.sessions, state.activeSession]);

  useEffect(() => {
    Utils.initTheme();
    const handleStorageChange = (e) => {
      // Ignore null events
      if (!e.key) return;

      const subdomain = state.activeSession;
      if (e.key.startsWith('visited_') || e.key === "deleteAll") {
        setState((prevState) => {
          const newSessions = { ...prevState.sessions };
          if (e.key === "deleteAll") {
            if (newSessions[subdomain]) {
              newSessions[subdomain] = {
                ...newSessions[subdomain],
                httpRequests: [],
                dnsRequests: [],
                requests: {},
                visited: {},
                selectedRequest: undefined
              };
            }
          } else {
            const targetSubdomain = e.key.replace('visited_', '');
            if (newSessions[targetSubdomain]) {
              try {
                const newVisited = JSON.parse(e.newValue || "{}");
                newSessions[targetSubdomain] = {
                  ...newSessions[targetSubdomain],
                  visited: newVisited,
                  requests: Object.fromEntries(
                    Object.entries(newSessions[targetSubdomain].requests)
                      .map(([key, request]) => [
                        key,
                        newVisited[key] ? { ...request, new: false } : request
                      ])
                  )
                };
              } catch (err) {
                console.error('Error parsing visited data:', err);
              }
            }
          }
          return { ...prevState, sessions: newSessions };
        });
      } else if (e.key.startsWith('token_')) {
        // Only reload if this is a manual token change, not our automatic token refresh
        if (!e.newValue || e.newValue === "") {
          console.log("Manual token removal detected, reloading");
          document.location.reload();
        }
      } else if (e.key?.startsWith('lastSelectedRequest_')) {
        const targetSubdomain = e.key.replace('lastSelectedRequest_', '');
        setState((prevState) => {
          const newSessions = { ...prevState.sessions };
          if (newSessions[targetSubdomain] && newSessions[targetSubdomain].requests[e.newValue]) {
            newSessions[targetSubdomain].selectedRequest = e.newValue;
            newSessions[targetSubdomain].requests[e.newValue].new = false;
          }
          return { ...prevState, sessions: newSessions };
        });
      } else if (e.key?.startsWith('lastDeletedRequest_')) {
        const targetSubdomain = e.key.replace('lastDeletedRequest_', '');
        const requestId = e.newValue;
        setState((prevState) => {
          const newSessions = { ...prevState.sessions };
          if (newSessions[targetSubdomain]) {
            delete newSessions[targetSubdomain].requests[requestId];
            delete newSessions[targetSubdomain].visited[requestId];
            newSessions[targetSubdomain].httpRequests = newSessions[targetSubdomain].httpRequests
              .filter(value => value["_id"] !== requestId);
            newSessions[targetSubdomain].dnsRequests = newSessions[targetSubdomain].dnsRequests
              .filter(value => value["_id"] !== requestId);
          }
          return { ...prevState, sessions: newSessions };
        });
      }
    };

    window.addEventListener("storage", handleStorageChange);
    return () => {
      window.removeEventListener("storage", handleStorageChange);
    };
  }, [state.activeSession]); // Add dependency on activeSession

  const markAllAsVisited = () => {
    setState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const activeSession = newSessions[state.activeSession];
      
      if (activeSession) {
        const updatedRequests = {};
        const visited = {};

        Object.entries(activeSession.requests).forEach(([key, value]) => {
          updatedRequests[key] = { ...value, new: false };
          visited[key] = true;
        });

        activeSession.requests = updatedRequests;
        activeSession.visited = visited;
        
        localStorage.setItem(`visited_${state.activeSession}`, JSON.stringify(visited));
      }
      
      return { ...prevState, sessions: newSessions };
    });
  };

  const clickRequestAction = (action, id) => {
    setState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const activeSession = newSessions[state.activeSession];
      
      if (!activeSession) return prevState;

      if (action === "select") {
        if (activeSession.requests[id] !== undefined) {
          activeSession.selectedRequest = id;
          activeSession.requests[id]["new"] = false;
          if (activeSession.visited[id] === undefined) {
            activeSession.visited[id] = true;
            localStorage.setItem(`visited_${state.activeSession}`, JSON.stringify(activeSession.visited));
          }
          localStorage.setItem(`lastSelectedRequest_${state.activeSession}`, id);
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

        if (id === localStorage.getItem(`lastSelectedRequest_${state.activeSession}`)) {
          localStorage.setItem(`lastSelectedRequest_${state.activeSession}`, nextSelectedId);
          activeSession.selectedRequest = nextSelectedId;
        }

        Utils.deleteRequest(id).then(() => {
          localStorage.setItem(`visited_${state.activeSession}`, JSON.stringify(activeSession.visited));
          localStorage.setItem(`lastDeletedRequest_${state.activeSession}`, id);
        });
      } else if (action === "reset") {
        activeSession.selectedRequest = undefined;
        localStorage.setItem(`lastSelectedRequest_${state.activeSession}`, undefined);
      }

      return { ...prevState, sessions: newSessions };
    });
  };

  const updateSearchValue = (val) => {
    setState((prevState) => ({ ...prevState, searchValue: val }));
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

  const handleNewSession = async () => {
    try {
      const activeSession = state.sessions[state.activeSession];
      if (!activeSession) {
        throw new Error('No active session found');
      }

      // Get new subdomain and token
      const { subdomain, token } = await Utils.getRandomSubdomain();
      
      // Get current sessions array from localStorage
      const sessionsStr = localStorage.getItem('sessions');
      const sessions = JSON.parse(sessionsStr || '[]');
      
      // Update the session in localStorage
      const sessionIndex = sessions.findIndex(s => s.subdomain === activeSession.subdomain);
      if (sessionIndex !== -1) {
        sessions[sessionIndex] = {
          ...sessions[sessionIndex],
          subdomain,
          token
        };
        localStorage.setItem('sessions', JSON.stringify(sessions));
      }

      setState(prevState => {
        const newSessions = { ...prevState.sessions };
        delete newSessions[activeSession.subdomain];

        // Update the active session with new subdomain
        newSessions[subdomain] = {
          ...activeSession,
          url: `${subdomain}.${Utils.siteUrl}`,
          subdomain: subdomain,
          token: token
        };

        // Clean up old session data from localStorage
        localStorage.removeItem(`visited_${activeSession.subdomain}`);
        localStorage.removeItem(`lastSelectedRequest_${activeSession.subdomain}`);
        localStorage.removeItem(`token_${activeSession.subdomain}`);

        // Send session update to WebSocket
        if (websocketRef.current?.readyState === WebSocket.OPEN) {
          websocketRef.current.send(JSON.stringify({
            cmd: 'unregister_session',
            subdomain: activeSession.subdomain
          }));
          websocketRef.current.send(JSON.stringify({
            cmd: 'register_session',
            token: token,
            subdomain: subdomain
          }));
        }

        return {
          ...prevState,
          sessions: newSessions,
          activeSession: subdomain
        };
      });

      toast.success('Session URL updated successfully');
    } catch (error) {
      toast.error(`Failed to update session URL: ${error.message}`);
    }
  };

  // Use effect to handle side effects after deleteAllRequests
  useEffect(() => {
    if (state.deleteFlag) {
      localStorage.setItem("visited", "{}");
      localStorage.setItem("deleteAll", genRanHex(16));
    }
  }, [state.deleteFlag]);

  const deleteAllRequests = () => {
    setState((prevState) => {
      const newSessions = { ...prevState.sessions };
      const activeSession = newSessions[state.activeSession];
      
      if (activeSession) {
        activeSession.httpRequests = [];
        activeSession.dnsRequests = [];
        activeSession.requests = {};
        activeSession.visited = {};
      }
      
      return {
        ...prevState,
        sessions: newSessions,
        deleteFlag: !prevState.deleteFlag, // toggle flag to trigger useEffect
      };
    });
  };

  const onToggleMenu = (event) => {
    event.preventDefault();
    setState((prevState) => {
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

  // Add debug logging for state updates
  useEffect(() => {
    console.log('State updated:', new Date().toISOString());
  }, [state]);

  // Component rendering logic
  return (
    <div className="layout-wrapper layout-static">
      <AppTopbar
        onToggleMenu={onToggleMenu}
        updateSearchValue={updateSearchValue}
        sessions={state.sessions}
        activeSession={state.activeSession}
        onSessionChange={(session) => setState(prev => {
          try {
            // Get the session data from localStorage
            const sessionsStr = localStorage.getItem('sessions');
            if (!sessionsStr) return prev;

            const sessions = JSON.parse(sessionsStr);
            const sessionData = sessions.find(s => s.subdomain === session);
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
              token: sessionData.token
            };

            return {
              ...prev,
              sessions: {
                ...prev.sessions,
                [session]: existingSession
              },
              activeSession: session
            };
          } catch (error) {
            console.error('Error updating session state:', error);
            return prev;
          }
        })}
        onSessionRemove={(subdomain) => {
          setState(prev => {
            try {
              const newSessions = { ...prev.sessions };
              
              // Don't allow removing the last session
              if (Object.keys(newSessions).length <= 1) {
                toast.warn('Cannot remove the last session');
                return prev;
              }

              // Update sessions array in localStorage
              const sessionsStr = localStorage.getItem('sessions');
              if (sessionsStr) {
                const sessions = JSON.parse(sessionsStr);
                const updatedSessions = sessions.filter(s => s.subdomain !== subdomain);
                localStorage.setItem('sessions', JSON.stringify(updatedSessions));

                // Update selectedSessionIndex if needed
                let selectedIndex = parseInt(localStorage.getItem('selectedSessionIndex') || '0');
                if (selectedIndex >= updatedSessions.length) {
                  selectedIndex = Math.max(0, updatedSessions.length - 1);
                  localStorage.setItem('selectedSessionIndex', selectedIndex.toString());
                }
              }

              delete newSessions[subdomain];

              // Determine new active session
              let newActiveSession = prev.activeSession;
              if (subdomain === prev.activeSession) {
                const remainingSessions = Object.keys(newSessions);
                newActiveSession = remainingSessions[0];
              }

              // Notify WebSocket about session removal
              if (websocketRef.current?.readyState === WebSocket.OPEN) {
                websocketRef.current.send(JSON.stringify({
                  cmd: 'unregister_session',
                  subdomain: subdomain
                }));
              }

              return {
                ...prev,
                sessions: newSessions,
                activeSession: newActiveSession
              };
            } catch (error) {
              console.error('Error removing session:', error);
              return prev;
            }
          });
        }}
      />

      <AppSidebar
        user={state.sessions[state.activeSession]}
        searchValue={state.searchValue}
        clickRequestAction={clickRequestAction}
        deleteAllRequests={deleteAllRequests}
        markAllAsVisited={markAllAsVisited}
        activeSession={state.activeSession}
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
                    value={state.sessions[state.activeSession] ? 
                      `${state.sessions[state.activeSession].subdomain}.${state.sessions[state.activeSession].domain}` : 
                      ''}
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
                    onClick={handleNewSession}
                  />
                </div>
              }
            />
            <Routes>
              <Route
                exact
                path="/"
                element={<RequestsPage user={state.sessions[state.activeSession]} />}
              />
              <Route
                path="/requests"
                element={<RequestsPage user={state.sessions[state.activeSession]} />}
              />
              <Route
                path="/edit-response"
                element={
                  <EditResponsePage
                    content={state.response.raw}
                    statusCode={state.response.status_code}
                    headers={state.response.headers}
                    user={state.sessions[state.activeSession]}
                    fetched={state.response.fetched}
                    toast={toast}
                  />
                }
              />
              <Route
                path="/dns-settings"
                element={
                  <DnsSettingsPage
                    user={state.sessions[state.activeSession]}
                    dnsRecords={state.dnsRecords}
                    toast={toast}
                    fetched={state.dnsFetched}
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

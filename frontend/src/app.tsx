import React, { useState, useEffect, useRef, useCallback } from "react";
import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { AppTopbar } from "./components/topbar";
import { AppSidebar } from "./components/sidebar";
import { RequestsPage } from "./components/requests-page";
import { EditResponsePage } from "./components/edit-response-page";
import { DnsSettingsPage } from "./components/dns-settings-page";
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

import { AppState, AppSession } from "./types/app-types";
import { useWebSocketService } from "./services/websocket-service";
import { useSessionManager } from "./hooks/useSessionManager";
import { useRequestHandler } from "./hooks/useRequestHandler";

const App: React.FC = () => {
  const urlArea = useRef<HTMLInputElement | null>(null);
  const location = useLocation();
  const navigate = useNavigate();
  const [sharedRequest, setSharedRequest] = useState<string | null>(null);
  const [themeState, setThemeState] = useState(Utils.getTheme());

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

  const ws_url = `/api/ws2`;

  const {
    initializeSessions,
    handleNewURL: sessionHandleNewURL,
    getWebSocketSessions,
  } = useSessionManager();

  const {
    handleMessage,
    clickRequestAction,
    markAllAsVisited,
    deleteAllRequests,
  } = useRequestHandler(appState.sessions, appState.activeSession);

  const { sendMessage, registerSessions } = useWebSocketService({
    url: ws_url,
    onMessage: handleMessage,
    onOpen: () => {
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
    },
    sessions: getWebSocketSessions(),
    debug: false,
  });

  useEffect(() => {
    initializeSessions();
  }, [initializeSessions]);

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

                registerSessions([{ token: data.token, subdomain: sessionId }]);

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
                      dnsRecords: [],
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
  }, [location, navigate, sendMessage, registerSessions]);

  useEffect(() => {
    const handleThemeChange = () => {
      setThemeState(Utils.getTheme());
    };
    window.addEventListener("themeChange", handleThemeChange);
    return () => window.removeEventListener("themeChange", handleThemeChange);
  }, []);

  useEffect(() => {
    if (sharedRequest && appState.activeSession) {
      const session = appState.sessions[appState.activeSession];
      if (session && session.requests[sharedRequest]) {
        clickRequestAction("select", sharedRequest);
        setSharedRequest(null);
      }
    }
  }, [
    sharedRequest,
    appState.activeSession,
    appState.sessions,
    clickRequestAction,
  ]);

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
      const { subdomain, token } = await sessionHandleNewURL();

      registerSessions([{ token, subdomain }]);

      toast.success(`Created new URL: ${subdomain}`, Utils.toastOptions);
    } catch (error) {
      console.error("Failed to create new URL:", error);
      toast.error("Failed to create new URL", Utils.toastOptions);
    }
  }, [sessionHandleNewURL, registerSessions]);

  const onToggleMenu = useCallback(() => {
    setAppState((prevState) => ({
      ...prevState,
      staticMenuInactive: !prevState.staticMenuInactive,
    }));
  }, []);

  const currentSessionData = appState.activeSession
    ? appState.sessions[appState.activeSession]
    : null;

  return (
    <div
      className={`layout-wrapper layout-static ${appState.staticMenuInactive ? "layout-static-sidebar-inactive" : ""} ${themeState}`}
    >
      <AppTopbar
        onToggleMenu={onToggleMenu}
        staticMenuInactive={appState.staticMenuInactive}
        sessions={appState.sessions as Record<string, AppSession>}
        activeSession={appState.activeSession}
        copyUrl={copyUrl}
        copyDomain={copyDomain}
        handleNewURL={handleNewURL}
      />

      <div className="layout-main">
        <div className="layout-content">
          <div className="layout-sidebar-container">
            <AppSidebar
              user={currentSessionData}
              activeSession={appState.activeSession}
              sessions={appState.sessions}
              searchValue={appState.searchValue}
              deleteAllRequests={deleteAllRequests}
              markAllAsVisited={markAllAsVisited}
              clickRequestAction={clickRequestAction}
              updateSearchValue={updateSearchValue}
            />
          </div>

          <div className="layout-main-content">
            <div className="grid">
              <div className="col-12">
                {/* Removed toolbar completely for more compact design */}
              </div>
            </div>
            <Routes>
              <Route
                path="/"
                element={
                  <RequestsPage
                    user={currentSessionData}
                    toast={toast}
                    activeSession={appState.activeSession}
                  />
                }
              />
              <Route
                path="/edit-response"
                element={
                  <EditResponsePage
                    user={currentSessionData}
                    toast={toast}
                    activeSession={appState.activeSession}
                  />
                }
              />
              <Route
                path="/dns-settings"
                element={
                  <DnsSettingsPage
                    user={currentSessionData}
                    toast={toast}
                    activeSession={appState.activeSession}
                    dnsRecords={appState.dnsRecords}
                  />
                }
              />
            </Routes>
          </div>
        </div>
      </div>

      <ToastContainer />

      <input
        type="text"
        ref={urlArea}
        style={{ position: "absolute", left: "-9999px" }}
        readOnly
        value={
          currentSessionData
            ? `${currentSessionData.subdomain}.${currentSessionData.domain}`
            : ""
        }
      />
    </div>
  );
};

export default App;

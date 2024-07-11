import React, { useState, useEffect, useRef, useCallback } from "react";
import { AppTopbar } from "./components/topbar";
import { AppSidebar } from "./components/sidebar";
import { Route, Routes } from "react-router-dom";
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

const App = () => {
  const urlArea = useRef(null);
  const websocketRef = useRef(null);
  const [state, setState] = useState({
    layoutMode: "static",
    layoutColorMode: "light",
    staticMenuInactive: false,
    overlayMenuActive: false,
    mobileMenuActive: false,
    user: {
      url: Utils.getUserURL(),
      domain: Utils.siteUrl,
      subdomain: Utils.subdomain,
      httpRequests: [],
      dnsRequests: [],
      timestamp: null,
      requests: {},
      visited: JSON.parse(localStorage.getItem("visited") || "{}"),
      selectedRequest: localStorage.getItem("lastSelectedRequest"),
    },
    searchValue: "",
    response: { raw: "", headers: [], status_code: 200, fetched: false },
    dnsRecords: [],
    dnsFetched: false,
  });

  const updateTitle = useCallback(() => {
    const { user } = state;
    const n =
      user.httpRequests.length +
      user.dnsRequests.length -
      Object.keys(user.visited).length;
    const text = `Dashboard - ${Utils.siteUrl}`;
    document.title = n <= 0 ? text : `(${n}) ${text}`;
  }, [state]);

  const initWebSocket = useCallback(
    (ws_url) => {
      if (websocketRef.current) {
        return; // If WebSocket is already initialized, return
      }

      const socket = new WebSocket(ws_url);
      websocketRef.current = socket;

      socket.onopen = () => {
        const user = state.user;
        user.httpRequests = [];
        user.dnsRequests = [];
        user.requests = {};
        socket.send(localStorage.getItem("token"));
      };

      socket.onmessage = (event) => {
        const data = JSON.parse(event.data);
        const cmd = data["cmd"];
        if (cmd === "requests" || cmd === "request") {
          setState((prevState) => {
            const newUser = { ...prevState.user };
            if (cmd === "requests") {
              const requests = data["data"].map((r) => JSON.parse(r));
              const httpRequests = requests.filter((r) => r["type"] === "http");
              const dnsRequests = requests.filter((r) => r["type"] === "dns");

              httpRequests.forEach((httpRequest) => {
                newUser.httpRequests.push(httpRequest);
                newUser.requests[httpRequest["_id"]] = httpRequest;
                httpRequest["new"] = false;
              });

              dnsRequests.forEach((dnsRequest) => {
                newUser.dnsRequests.push(dnsRequest);
                newUser.requests[dnsRequest["_id"]] = dnsRequest;
                dnsRequest["new"] = false;
              });
            } else if (cmd === "request") {
              const request = JSON.parse(data["data"]);
              request["new"] = true;
              if (request["type"] === "http") {
                newUser.httpRequests.push(request);
              } else if (request["type"] === "dns") {
                newUser.dnsRequests.push(request);
              }
              newUser.requests[request["_id"]] = request;
            }

            return { ...prevState, user: newUser };
          });
        }
      };

      socket.onclose = () => {
        websocketRef.current = null; // Reset the WebSocket ref
        setTimeout(() => {
          initWebSocket(ws_url);
        }, 1000);
      };

      socket.onerror = (error) => {
        console.error("WebSocket error:", error);
        socket.close();
      };
    },
    [state],
  );

  useEffect(() => {
    if (!Utils.userHasSubdomain()) {
      Utils.getRandomSubdomain();
      return;
    }

    Utils.initTheme();

    const handleStorageChange = (e) => {
      if (e.key === "visited" || e.key === "deleteAll") {
        let newVisited = e.newValue;
        if (e.key === "deleteAll") newVisited = "{}";
        setState((prevState) => {
          const newUser = {
            ...prevState.user,
            visited: JSON.parse(newVisited),
          };
          Object.entries(newUser.visited).forEach(([key]) => {
            if (newUser.requests[key]) {
              newUser.requests[key]["new"] = false;
            }
          });

          if (newVisited === "{}") {
            newUser.httpRequests = [];
            newUser.dnsRequests = [];
            newUser.requests = {};
            newUser.selectedRequest = undefined;
          }

          return { ...prevState, user: newUser };
        }, updateTitle);
      } else if (e.key === "token") {
        document.location.reload();
      } else if (e.key === "lastSelectedRequest") {
        const id = e.newValue;
        setState((prevState) => {
          const newUser = { ...prevState.user };
          if (newUser.requests[id] !== undefined) {
            newUser.selectedRequest = id;
            newUser.requests[id]["new"] = false;
          }
          return { ...prevState, user: newUser };
        });
      } else if (e.key === "lastDeletedRequest") {
        const id = e.newValue;
        setState((prevState) => {
          const newUser = { ...prevState.user };
          delete newUser.requests[id];
          delete newUser.visited[id];

          newUser.httpRequests = newUser.httpRequests.filter(
            (value) => value["_id"] !== id,
          );
          newUser.dnsRequests = newUser.dnsRequests.filter(
            (value) => value["_id"] !== id,
          );

          return { ...prevState, user: newUser };
        }, updateTitle);
      }
    };

    window.addEventListener("storage", handleStorageChange);
    return () => {
      window.removeEventListener("storage", handleStorageChange);
    };
  }, [updateTitle]);

  // WebSocket connection effect
  useEffect(() => {
    const protocol = document.location.protocol === "https:" ? "wss" : "ws";
    const ws_url = `${protocol}://${document.location.host}/api/ws`;

    initWebSocket(ws_url);

    return () => {
      if (websocketRef.current) {
        websocketRef.current.close();
      }
    };
  }, [initWebSocket]);

  const markAllAsVisited = () => {
    const updatedRequests = {};
    const visited = {};

    Object.entries(state.user.requests).forEach(([key, value]) => {
      updatedRequests[key] = { ...value, new: false };
      visited[key] = true;
    });

    setState(
      (prevState) => ({
        ...prevState,
        user: { ...prevState.user, requests: updatedRequests, visited },
      }),
      () => {
        localStorage.setItem("visited", JSON.stringify(visited));
        updateTitle();
      },
    );
  };

  const clickRequestAction = (action, id) => {
    setState((prevState) => {
      const newUser = { ...prevState.user };

      if (action === "select") {
        if (newUser.requests[id] !== undefined) {
          newUser.selectedRequest = id;
          newUser.requests[id]["new"] = false;
          if (newUser.visited[id] === undefined) {
            newUser.visited[id] = true;
            localStorage.setItem("visited", JSON.stringify(newUser.visited));
          }
          localStorage.setItem("lastSelectedRequest", id);
        }
      } else if (action === "delete") {
        const combinedRequests = [
          ...newUser.httpRequests,
          ...newUser.dnsRequests,
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

        delete newUser.requests[id];
        delete newUser.visited[id];

        newUser.httpRequests = newUser.httpRequests.filter(
          (request) => request["_id"] !== id,
        );
        newUser.dnsRequests = newUser.dnsRequests.filter(
          (request) => request["_id"] !== id,
        );

        if (id === localStorage.getItem("lastSelectedRequest")) {
          localStorage.setItem("lastSelectedRequest", nextSelectedId);
          newUser.selectedRequest = nextSelectedId;
        }

        Utils.deleteRequest(id).then(() => {
          localStorage.setItem("visited", JSON.stringify(newUser.visited));
          localStorage.setItem("lastDeletedRequest", id);
        });
      } else if (action === "reset") {
        newUser.selectedRequest = undefined;
        localStorage.setItem("lastSelectedRequest", undefined);
      }

      return { ...prevState, user: newUser };
    }, updateTitle);
  };

  const updateSearchValue = (val) => {
    setState((prevState) => ({ ...prevState, searchValue: val }));
  };

  const copyUrl = () => {
    if (!navigator.clipboard) {
      urlArea.current.select();
      document.execCommand("copy");
    } else {
      navigator.clipboard.writeText(urlArea.current.value);
    }
    toast.info("URL copied to clipboard!", {
      position: "bottom-center",
      autoClose: 2500,
      hideProgressBar: false,
      closeOnClick: true,
      pauseOnHover: true,
      draggable: true,
    });
  };

  const newUrl = () => {
    Utils.getRandomSubdomain();
  };

  const deleteAllRequests = () => {
    setState(
      {
        ...state,
        user: {
          ...state.user,
          httpRequests: [],
          dnsRequests: [],
          requests: {},
          visited: {},
        },
      },
      () => {
        const genRanHex = (size) =>
          [...Array(size)]
            .map(() => Math.floor(Math.random() * 16).toString(16))
            .join("");

        localStorage.setItem("visited", "{}");
        localStorage.setItem("deleteAll", genRanHex(16));

        updateTitle();
      },
    );
  };

  const onToggleMenu = (event) => {
    event.preventDefault();
    setState((prevState) => {
      const isDesktop = window.innerWidth > 1024;
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

  return (
    <div className="layout-wrapper layout-static">
      <AppTopbar
        onToggleMenu={onToggleMenu}
        updateSearchValue={updateSearchValue}
      />

      <AppSidebar
        user={state.user}
        searchValue={state.searchValue}
        clickRequestAction={clickRequestAction}
        deleteAllRequests={deleteAllRequests}
        markAllAsVisited={markAllAsVisited}
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
                    value={state.user.url}
                    style={{ width: "300px", marginRight: "1em" }}
                    ref={urlArea}
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
                    onClick={newUrl}
                  />
                </div>
              }
            />
            <Routes>
              <Route
                exact
                path="/"
                element={<RequestsPage user={state.user} />}
              />
              <Route
                path="/requests"
                element={<RequestsPage user={state.user} />}
              />
              <Route
                path="/edit-response"
                element={
                  <EditResponsePage
                    content={state.response.raw}
                    statusCode={state.response.status_code}
                    headers={state.response.headers}
                    user={state.user}
                    fetched={state.response.fetched}
                    toast={toast}
                  />
                }
              />
              <Route
                path="/dns-settings"
                element={
                  <DnsSettingsPage
                    user={state.user}
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

      <ToastContainer theme={Utils.getTheme()} />
      <div className="layout-mask"></div>
    </div>
  );
};

export default App;

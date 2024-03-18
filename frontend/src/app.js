import React, { Component } from "react";
import classNames from "classnames";
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

class App extends Component {
  constructor() {
    super();

    if (!Utils.userHasSubdomain()) {
      Utils.getRandomSubdomain();
      return;
    }

    this.user = {
      url: Utils.getUserURL(),
      domain: Utils.siteUrl,
      subdomain: Utils.subdomain,
      httpRequests: [],
      dnsRequests: [],
      timestamp: null,
      requests: {},
      visited: {},
      selectedRequest: localStorage.getItem("lastSelectedRequest"),
    };

    this.state = {
      layoutMode: "static",
      layoutColorMode: "light",
      staticMenuInactive: false,
      overlayMenuActive: false,
      mobileMenuActive: false,
      user: this.user,
      searchValue: "",
      response: { raw: "", headers: [], status_code: 200, fetched: false },
      dnsRecords: [],
      dnsFetched: false,
    };

    Utils.initTheme();

    this.user.visited = JSON.parse(
      localStorage.getItem("visited") === null
        ? "{}"
        : localStorage.getItem("visited"),
    );

    let protocol = "ws";
    if (document.location.protocol === "https:") {
      protocol = "wss";
    }

    let ws_url = `${protocol}://${document.location.host}/api/ws`;
    if (
      (document.location.hostname === "localhost" ||
        document.location.hostname === "127.0.0.1") &&
      window.location.port === "3000"
    ) {
      ws_url = `${protocol}://localhost:21337/api/ws`;
    }

    let app = this;

    window.addEventListener("storage", (e) => {
      if (e.key === "visited" || e.key === "deleteAll") {
        let newVisited = e.newValue;
        if (e.key === "deleteAll") newVisited = "{}";
        app.setState(
          (prevState) => {
            const newUser = JSON.parse(JSON.stringify(prevState.user));
            newUser.visited = JSON.parse(newVisited);

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

            return { user: newUser };
          },
          () => {
            app.updateTitle();
          },
        );
      } else if (e.key === "token") {
        document.location.reload();
      } else if (e.key === "lastSelectedRequest") {
        const id = e.newValue;

        app.setState((prevState) => {
          const newUser = JSON.parse(JSON.stringify(prevState.user));
          if (newUser.requests[id] !== undefined) {
            newUser.selectedRequest = id;
            newUser.requests[id]["new"] = false;
          }
          return { user: newUser };
        });
      } else if (e.key === "lastDeletedRequest") {
        const id = e.newValue;

        app.setState(
          (prevState) => {
            const newUser = JSON.parse(JSON.stringify(prevState.user));
            delete newUser.requests[id];
            delete newUser.visited[id];

            newUser.httpRequests = newUser.httpRequests.filter(
              (value) => value["_id"] !== id,
            );
            newUser.dnsRequests = newUser.dnsRequests.filter(
              (value) => value["_id"] !== id,
            );

            return { user: newUser };
          },
          () => {
            app.updateTitle();
          },
        );
      }
    });

    function initWebSocket(ws_url) {
      let socket = new WebSocket(ws_url);
      socket.onopen = function (event) {
        // Send the token to the WebSocket server
        app.state.user["httpRequests"] = [];
        app.state.user["dnsRequests"] = [];
        app.state.user["requests"] = {};
        socket.send(localStorage.getItem("token"));
      };

      // Event handler for incoming WebSocket messages
      socket.onmessage = function (event) {
        event = JSON.parse(event.data);
        let cmd = event["cmd"];

        if (cmd === "requests" || cmd === "request") {
          app.setState(
            (prevState) => {
              // Deep copy the user object to avoid direct mutation
              let newUser = JSON.parse(JSON.stringify(prevState.user));

              if (cmd === "requests") {
                let requests = event["data"].map((r) => JSON.parse(r));
                let httpRequests = requests.filter((r) => r["type"] === "http");
                let dnsRequests = requests.filter((r) => r["type"] === "dns");

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
                let request = JSON.parse(event["data"]);
                request["new"] = true;
                if (request["type"] === "http") {
                  newUser.httpRequests.push(request);
                } else if (request["type"] === "dns") {
                  newUser.dnsRequests.push(request);
                }
                newUser.requests[request["_id"]] = request;
              }

              // Return new state
              return { user: newUser };
            },
            () => {
              app.updateTitle();
            },
          );
        }
      };

      // Event handler for WebSocket connection closure
      socket.onclose = function () {
        setTimeout(function () {
          initWebSocket(ws_url);
        }, 1000);
      };

      app.socket = socket;
    }

    initWebSocket(ws_url);

    Utils.getFile()
      .then((res) => {
        try {
          let decoded = Utils.base64DecodeUnicode(res.raw);
          res.raw = decoded;
        } catch {}

        res.fetched = true;
        this.setState({ response: res });
        this.setState(this.state);
      })
      .catch((err) => {
        if (err.response.status === 403) {
          localStorage.removeItem("token");
          document.location.reload();
        }
      });

    Utils.getDNSRecords()
      .then((res) => {
        this.setState({ dnsRecords: res });
        this.setState({ dnsFetched: true });
      })
      .catch((err) => {
        if (err.response.status === 403) {
          localStorage.removeItem("token");
          document.location.reload();
        }
      });

    this.onWrapperClick = this.onWrapperClick.bind(this);
    this.onToggleMenu = this.onToggleMenu.bind(this);
    this.onSidebarClick = this.onSidebarClick.bind(this);
    this.onMenuItemClick = this.onMenuItemClick.bind(this);
    this.clickRequestAction = this.clickRequestAction.bind(this);
    this.copyUrl = this.copyUrl.bind(this);
    this.newUrl = this.newUrl.bind(this);
    this.updateTitle = this.updateTitle.bind(this);
    this.updateSearchValue = this.updateSearchValue.bind(this);
    this.deleteAllRequests = this.deleteAllRequests.bind(this);
    this.markAllAsVisited = this.markAllAsVisited.bind(this);
  }

  markAllAsVisited() {
    const updatedRequests = {};
    const visited = {};

    // Copying the requests to a new object
    for (const [key, value] of Object.entries(this.state.user.requests)) {
      updatedRequests[key] = { ...value, new: false };
      visited[key] = true;
    }

    // Updating the state with the new objects
    this.setState(
      (prevState) => ({
        user: {
          ...prevState.user,
          requests: updatedRequests,
          visited: visited,
        },
      }),
      () => {
        localStorage.setItem("visited", JSON.stringify(visited));
        this.updateTitle();
      },
    );
  }

  clickRequestAction(action, id) {
    this.setState(
      (prevState) => {
        // Deep copy the user object to avoid direct mutation
        let newUser = JSON.parse(JSON.stringify(prevState.user));

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
          // Combine httpRequests and dnsRequests to find the next or previous request ID
          const combinedRequests = [
            ...newUser.httpRequests,
            ...newUser.dnsRequests,
          ];

          // Find index of the request being deleted
          const deleteIndex = combinedRequests.findIndex(
            (request) => request["_id"] === id,
          );

          // Calculate the next index, or use the previous one if the deleted request is the last
          let nextSelectedIndex =
            deleteIndex >= combinedRequests.length - 1
              ? deleteIndex - 1
              : deleteIndex + 1;

          // Ensure the index is within bounds
          nextSelectedIndex = Math.max(
            0,
            Math.min(nextSelectedIndex, combinedRequests.length - 1),
          );

          // Get the next selected request ID, or undefined if no requests are left
          const nextSelectedId =
            combinedRequests.length > 0
              ? combinedRequests[nextSelectedIndex]["_id"]
              : undefined;

          // Perform deletion
          delete newUser.requests[id];
          delete newUser.visited[id];

          newUser.httpRequests = newUser.httpRequests.filter(
            (request) => request["_id"] !== id,
          );
          newUser.dnsRequests = newUser.dnsRequests.filter(
            (request) => request["_id"] !== id,
          );

          // Set the next selected request
          if (id === localStorage.getItem("lastSelectedRequest")) {
            localStorage.setItem("lastSelectedRequest", nextSelectedId);
            newUser.selectedRequest = nextSelectedId;
          }

          Utils.deleteRequest(id).then((res) => {
            localStorage.setItem("visited", JSON.stringify(newUser.visited));
            localStorage.setItem("lastDeletedRequest", id);
          });
        } else if (action === "reset") {
          newUser.selectedRequest = undefined;
          localStorage.setItem("lastSelectedRequest", undefined);
        }

        // Return new state
        return { user: newUser };
      },
      () => {
        this.updateTitle();
      },
    );
  }

  isDesktop() {
    return window.innerWidth > 1024;
  }

  updateTitle() {
    let n =
      this.state.user.httpRequests.length +
      this.state.user.dnsRequests.length -
      Object.keys(this.state.user.visited).length;
    let text = "Dashboard - " + Utils.siteUrl;
    if (n <= 0) {
      document.title = text;
    } else {
      document.title = "(" + n + ") " + text;
    }
  }

  updateSearchValue(val) {
    this.setState((prevState) => ({
      ...prevState,
      searchValue: val,
    }));
  }

  copyUrl(e) {
    if (!navigator.clipboard) {
      this.urlArea.select();
      document.execCommand("copy");
    } else {
      navigator.clipboard.writeText(this.urlArea.value);
    }
    toast.info("URL copied to clipboard!", {
      position: "bottom-center",
      autoClose: 2500,
      hideProgressBar: false,
      closeOnClick: true,
      pauseOnHover: true,
      draggable: true,
    });
  }

  newUrl(e) {
    Utils.getRandomSubdomain();
  }

  componentDidMount() {}

  componentWillUnmount() {
    clearInterval(this.interval);
  }

  componentDidUpdate() {
    if (this.state.mobileMenuActive)
      this.addClass(document.body, "body-overflow-hidden");
    else this.removeClass(document.body, "body-overflow-hidden");
  }

  deleteAllRequests() {
    this.setState(
      {
        user: {
          ...this.state.user,
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

        this.updateTitle();
      },
    );
  }

  render() {
    const wrapperClass = classNames("layout-wrapper", {
      "layout-overlay": this.state.layoutMode === "overlay",
      "layout-static": this.state.layoutMode === "static",
      "layout-static-sidebar-inactive":
        this.state.staticMenuInactive && this.state.layoutMode === "static",
      "layout-overlay-sidebar-active":
        this.state.overlayMenuActive && this.state.layoutMode === "overlay",
      "layout-mobile-sidebar-active": this.state.mobileMenuActive,
    });

    return (
      <div className={wrapperClass} onClick={this.onWrapperClick}>
        <AppTopbar
          onToggleMenu={this.onToggleMenu}
          updateSearchValue={this.updateSearchValue}
        />

        <AppSidebar
          user={this.state.user}
          searchValue={this.state.searchValue}
          clickRequestAction={this.clickRequestAction}
          deleteAllRequests={this.deleteAllRequests}
          markAllAsVisited={this.markAllAsVisited}
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
                        href="#/edit-response"
                        label="Response"
                        icon="pi pi-pencil"
                        className="p-button-text p-button-secondary"
                        style={{ marginRight: ".25em" }}
                      />
                    </a>
                    <a href="#/dns-settings">
                      <Button
                        href="#/dns-settings"
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
                      value={this.state.user.url}
                      style={{ width: "300px", marginRight: "1em" }}
                      ref={(urlArea) => (this.urlArea = urlArea)}
                    />
                    <Button
                      label="Copy URL"
                      icon="pi pi-copy"
                      className="p-button-success"
                      style={{ marginRight: ".25em" }}
                      onClick={this.copyUrl}
                    />
                    <Button
                      label="New URL"
                      icon="pi pi-refresh"
                      onClick={this.newUrl}
                    />
                  </div>
                }
              />
              <Routes>
                <Route
                  exact
                  path="/"
                  element={<RequestsPage user={this.state.user} />}
                />
                <Route
                  path="/requests"
                  element={<RequestsPage user={this.state.user} />}
                />
                <Route
                  path="/edit-response"
                  element={
                    <EditResponsePage
                      content={this.state.response.raw}
                      statusCode={this.state.response["status_code"]}
                      headers={this.state.response.headers}
                      user={this.state.user}
                      fetched={this.state.response.fetched}
                      toast={toast}
                    />
                  }
                />
                <Route
                  path="/dns-settings"
                  element={
                    <DnsSettingsPage
                      user={this.state.user}
                      dnsRecords={this.state.dnsRecords}
                      toast={toast}
                      fetched={this.state.dnsFetched}
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
  }

  onWrapperClick(event) {
    if (!this.menuClick) {
      this.setState({
        overlayMenuActive: false,
        mobileMenuActive: false,
      });
    }

    this.menuClick = false;
  }

  onToggleMenu(event) {
    this.menuClick = true;

    if (this.isDesktop()) {
      if (this.state.layoutMode === "overlay") {
        this.setState({
          overlayMenuActive: !this.state.overlayMenuActive,
        });
      } else if (this.state.layoutMode === "static") {
        this.setState({
          staticMenuInactive: !this.state.staticMenuInactive,
        });
      }
    } else {
      const mobileMenuActive = this.state.mobileMenuActive;
      this.setState({
        mobileMenuActive: !mobileMenuActive,
      });
    }

    event.preventDefault();
  }

  onSidebarClick(event) {
    this.menuClick = true;
  }

  onMenuItemClick(event) {
    if (!event.item.items) {
      this.setState({
        overlayMenuActive: false,
        mobileMenuActive: false,
      });
    }
  }

  addClass(element, className) {
    if (element.classList) element.classList.add(className);
    else element.className += " " + className;
  }

  removeClass(element, className) {
    if (element.classList) element.classList.remove(className);
    else
      element.className = element.className.replace(
        new RegExp(
          "(^|\\b)" + className.split(" ").join("|") + "(\\b|$)",
          "gi",
        ),
        " ",
      );
  }
}

export default App;

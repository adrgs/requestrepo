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

    if (localStorage.getItem("theme") !== "dark" && localStorage.getItem("theme") !== "light") {
      // get system theme
      if (window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches) {
        localStorage.setItem("theme", "dark");
      } else {
        localStorage.setItem("theme", "light");
      }
    }

    if (localStorage.getItem("theme") === "dark") {
      document.body.classList.add("dark");
    }

    this.user.visited = JSON.parse(localStorage.getItem("visited") === null ? "{}" : localStorage.getItem("visited"));

    let protocol = "ws";
    if (document.location.protocol === "https:") {
      protocol = "wss";
    }

    let ws_url = `${protocol}://${document.location.host}/api/ws`;
    if ((document.location.hostname === "localhost" || document.location.hostname === "127.0.0.1") && window.location.port === "3000") {
      ws_url = `${protocol}://localhost:21337/api/ws`;
    }

    let user = this.user;
    let app = this;

    window.addEventListener("storage", (e) => {
      if (e.key === "visited") {
        app.state.user.visited = JSON.parse(e.newValue);
        for (const [key] of Object.entries(app.state.user.visited)) {
          if (app.state.user.requests[key]) {
            app.state.user.requests[key]["new"] = false;
          }
        }
        if (e.newValue === "{}") {
          app.state.user["httpRequests"] = [];
          app.state.user["dnsRequests"] = [];
          app.state.user["requests"] = {};
        }

        app.setState({ state: app.state });
        app.forceUpdate();
        app.updateTitle();
      } else if (e.key === "token") {
        document.location.reload();
      } else if (e.key === "lastSelectedRequest") {
        let id = e.newValue;
        if (app.state.user.requests[id] !== undefined) {
          app.state.user.selectedRequest = id;
          app.state.user.requests[id]["new"] = false;
          app.setState({ state: app.state });
        }
      } else if (e.key === "lastDeletedRequest") {
        let id = e.newValue;
        delete app.state.user.requests[id];
        delete app.state.user.visited[id];

        app.state.user.httpRequests = app.state.user.httpRequests.filter(function (value, index, arr) {
          return value["_id"] !== id;
        });
        app.state.user.dnsRequests = app.state.user.dnsRequests.filter(function (value, index, arr) {
          return value["_id"] !== id;
        });

        app.setState({ state: app.state });
        app.forceUpdate();
        app.updateTitle();
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
        if (cmd === "requests") {
          let requests = event["data"].map((r) => JSON.parse(r));

          let httpRequests = requests.filter((r) => r["type"] === "http");
          if (httpRequests.length > 0) {
            user["httpRequests"] = user["httpRequests"].concat(httpRequests);

            for (let i = 0; i < user["httpRequests"].length; i++) {
              user.requests[user["httpRequests"][i]["_id"]] = user["httpRequests"][i];
              user["httpRequests"][i]["new"] = false;
            }
          }

          let dnsRequests = requests.filter((r) => r["type"] === "dns");
          if (dnsRequests.length > 0) {
            user["dnsRequests"] = user["dnsRequests"].concat(dnsRequests);
            for (let i = 0; i < user["dnsRequests"].length; i++) {
              user.requests[user["dnsRequests"][i]["_id"]] = user["dnsRequests"][i];
              user["dnsRequests"][i]["new"] = false;
            }
          }

          app.updateTitle();
          app.setState({ state: app.state });
        } else if (cmd === "request") {
          let request = JSON.parse(event["data"]);
          request["new"] = true;
          if (request["type"] === "http") {
            app.user["httpRequests"] = app.user["httpRequests"].concat([request]);
          } else if (request["type"] === "dns") {
            app.user["dnsRequests"] = app.user["dnsRequests"].concat([request]);
          }
          app.user.requests[request["_id"]] = request;
          app.updateTitle();
          app.setState({ state: app.state });
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
    this.doRerender = this.doRerender.bind(this);
    this.markAllAsVisited = this.markAllAsVisited.bind(this);
  }

  markAllAsVisited() {
    let obj = {};
    for (let [key] of Object.entries(this.state.user.requests)) {
      this.state.user.requests[key]["new"] = false;
      obj[key] = true;
    }
    this.state.user.visited = obj;
    localStorage.setItem("visited", JSON.stringify(obj));
    this.updateTitle();
    this.setState({ user: this.state.user });
    this.forceUpdate();
  }

  clickRequestAction(action, id) {
    if (action === "select") {
      if (this.state.user.requests[id] !== undefined) {
        this.state.user.selectedRequest = id;
        this.state.user.requests[id]["new"] = false;
        if (this.state.user.visited[id] === undefined) {
          this.state.user.visited[id] = true;
          localStorage.setItem("visited", JSON.stringify(this.state.user.visited));
        }
        localStorage.setItem("lastSelectedRequest", id);
      }
    } else if (action === "delete") {
      delete this.state.user.requests[id];
      delete this.state.user.visited[id];

      this.state.user.httpRequests = this.state.user.httpRequests.filter(function (value, index, arr) {
        return value["_id"] !== id;
      });
      this.state.user.dnsRequests = this.state.user.dnsRequests.filter(function (value, index, arr) {
        return value["_id"] !== id;
      });

      this.setState({ user: this.state.user });

      Utils.deleteRequest(id).then((res) => {
        localStorage.setItem("visited", JSON.stringify(this.state.user.visited));
        localStorage.setItem("lastDeletedRequest", id);
      });
    } else if (action === "reset") {
      this.state.user.selectedRequest = undefined;
      localStorage.setItem("lastSelectedRequest", undefined);
    }
    this.updateTitle();
  }

  isDesktop() {
    return window.innerWidth > 1024;
  }

  updateTitle() {
    let n = this.state.user.httpRequests.length + this.state.user.dnsRequests.length - Object.keys(this.state.user.visited).length;
    let text = "Dashboard - " + Utils.siteUrl;
    if (n <= 0) {
      document.title = text;
    } else {
      document.title = "(" + n + ") " + text;
    }
  }

  updateSearchValue(val) {
    this.state.searchValue = val;
    this.setState({ state: this.state });
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
    if (this.state.mobileMenuActive) this.addClass(document.body, "body-overflow-hidden");
    else this.removeClass(document.body, "body-overflow-hidden");
  }

  doRerender() {
    this.forceUpdate();
  }

  render() {
    const wrapperClass = classNames("layout-wrapper", {
      "layout-overlay": this.state.layoutMode === "overlay",
      "layout-static": this.state.layoutMode === "static",
      "layout-static-sidebar-inactive": this.state.staticMenuInactive && this.state.layoutMode === "static",
      "layout-overlay-sidebar-active": this.state.overlayMenuActive && this.state.layoutMode === "overlay",
      "layout-mobile-sidebar-active": this.state.mobileMenuActive,
    });

    return (
      <div className={wrapperClass} onClick={this.onWrapperClick}>
        <AppTopbar onToggleMenu={this.onToggleMenu} updateSearchValue={this.updateSearchValue} />

        <AppSidebar user={this.state.user} searchValue={this.state.searchValue} clickRequestAction={this.clickRequestAction} doRerender={this.doRerender} markAllAsVisited={this.markAllAsVisited} />

        <div className="layout-main">
          <div className="grid">
            <div className="col-12">
              <Toolbar
                style={{ lineHeight: "3", borderRadius: "5px 5px 0px 0px" }}
                left={
                  <div style={{ textAlign: "center" }}>
                    <a href="#/requests">
                      <Button label="Requests" icon="pi pi-arrow-down" className="p-button-text p-button-secondary" style={{ marginRight: ".25em" }} />
                    </a>
                    <a href="#/edit-response">
                      <Button href="#/edit-response" label="Response" icon="pi pi-pencil" className="p-button-text p-button-secondary" style={{ marginRight: ".25em" }} />
                    </a>
                    <a href="#/dns-settings">
                      <Button href="#/dns-settings" label="DNS" icon="pi pi-home" className="p-button-text p-button-secondary" />
                    </a>
                  </div>
                }
                right={
                  <div style={{ textAlign: "center" }}>
                    <InputText type="text" placeholder="Your URL" value={this.state.user.url} style={{ width: "300px", marginRight: "1em" }} ref={(urlArea) => (this.urlArea = urlArea)} />
                    <Button label="Copy URL" icon="pi pi-copy" className="p-button-success" style={{ marginRight: ".25em" }} onClick={this.copyUrl} />
                    <Button label="New URL" icon="pi pi-refresh" onClick={this.newUrl} />
                  </div>
                }
              />
              <Routes>
                <Route exact path="/" element={<RequestsPage user={this.state.user} />} />
                <Route path="/requests" element={<RequestsPage user={this.state.user} />} />
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
                <Route path="/dns-settings" element={<DnsSettingsPage user={this.state.user} dnsRecords={this.state.dnsRecords} toast={toast} fetched={this.state.dnsFetched} />} />
              </Routes>
            </div>
          </div>
        </div>

        <ToastContainer />
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
    else element.className = element.className.replace(new RegExp("(^|\\b)" + className.split(" ").join("|") + "(\\b|$)", "gi"), " ");
  }
}

export default App;

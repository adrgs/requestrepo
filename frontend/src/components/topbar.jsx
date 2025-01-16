import React, { Component } from "react";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import { Utils } from "../utils";

export class AppTopbar extends Component {
  constructor(props) {
    super(props);
    this.state = {
      searchValue: "",
      themeToggler: false,
      sessions: props.sessions || {},
      activeSession: props.activeSession || "",
      unseenRequests: {},
    };
    this.handleSearchValueChange = this.handleSearchValueChange.bind(this);
    this.toggleTheme = this.toggleTheme.bind(this);
    this.handleSessionSelect = this.handleSessionSelect.bind(this);
    this.handleSessionRemove = this.handleSessionRemove.bind(this);
    this.handleNewSession = this.handleNewSession.bind(this);
  }

  toggleTheme() {
    Utils.toggleTheme();
    this.setState((prevState) => ({
      themeToggler: !prevState.themeToggler,
    }));
  }

  static defaultProps = {};

  static propTypes = {};

  componentDidUpdate(prevProps) {
    const sessionsChanged = prevProps.sessions !== this.props.sessions;
    const activeSessionChanged =
      prevProps.activeSession !== this.props.activeSession;

    if (sessionsChanged || activeSessionChanged) {
      const sessions = this.props.sessions || {};
      const activeSession = this.props.activeSession || "";

      const unseenRequests = Object.keys(this.props.sessions).reduce(
        (acc, subdomain) => ({
          ...acc,
          [subdomain]: this.calculateUnseenRequests(
            this.props.sessions[subdomain],
          ),
        }),
        {},
      );

      this.setState({
        sessions,
        activeSession,
        unseenRequests,
      });
    }
  }

  calculateUnseenRequests(session) {
    if (!session) return 0;
    const totalRequests =
      (session.httpRequests?.length || 0) + (session.dnsRequests?.length || 0);
    const visitedCount = Object.keys(session.visited || {}).length;
    return Math.max(0, totalRequests - visitedCount);
  }

  handleSessionSelect(subdomain) {
    if (this.props.onSessionChange) {
      this.props.onSessionChange(subdomain);
    }
  }

  handleSessionRemove(subdomain) {
    try {
      // Get and update sessions array
      const sessionsStr = localStorage.getItem("sessions");
      if (sessionsStr) {
        const sessions = JSON.parse(sessionsStr);
        const updatedSessions = sessions.filter(
          (s) => s.subdomain !== subdomain,
        );
        localStorage.setItem("sessions", JSON.stringify(updatedSessions));

        // Update selectedSessionIndex if needed
        let selectedIndex = parseInt(
          localStorage.getItem("selectedSessionIndex") || "0",
        );
        if (selectedIndex >= updatedSessions.length) {
          selectedIndex = Math.max(0, updatedSessions.length - 1);
          localStorage.setItem(
            "selectedSessionIndex",
            selectedIndex.toString(),
          );
        }
      }

      // Update state
      this.setState((prevState) => {
        const newSessions = { ...prevState.sessions };
        delete newSessions[subdomain];

        const newUnseenRequests = { ...prevState.unseenRequests };
        delete newUnseenRequests[subdomain];

        return {
          sessions: newSessions,
          unseenRequests: newUnseenRequests,
        };
      });

      if (this.props.onSessionRemove) {
        this.props.onSessionRemove(subdomain);
      }
    } catch (error) {
      console.error("Error removing session:", error);
    }
  }

  async handleNewSession() {
    try {
      // Get new subdomain and token
      const { subdomain, token } = await Utils.getRandomSubdomain();

      // Get current sessions array
      const sessionsStr = localStorage.getItem("sessions");
      const sessions = JSON.parse(sessionsStr || "[]");

      // Get the new session data
      const session = {
        subdomain,
        token,
        createdAt: new Date().toISOString(),
        unseenRequests: 0,
      };

      // Add new session to array
      sessions.push(session);

      // Update localStorage
      localStorage.setItem("sessions", JSON.stringify(sessions));

      // Update parent component
      if (this.props.onSessionChange) {
        this.props.onSessionChange(subdomain);
      }
    } catch (error) {
      console.error("Error creating new session:", error);
      throw error;
    }
  }

  handleSearchValueChange(event) {
    this.setState({ searchValue: event.target.value });
    this.props.updateSearchValue(event.target.value);
  }
  render() {
    const showTabs = Object.keys(this.state.sessions).length > 1;

    return (
      <div className="layout-topbar clearfix">
        <a href="/#" className="logo-link">
          <object data="/logo.svg" type="image/svg+xml" className="logo-object">
            requestrepo
          </object>
        </a>

        <div className="layout-topbar-session">
          {showTabs && (
            <div className="session-tabs">
              {Object.entries(this.state.sessions).map(
                ([subdomain, session]) => (
                  <div
                    key={subdomain}
                    className={`session-tab ${subdomain === this.state.activeSession ? "active" : ""}`}
                    onClick={() => this.handleSessionSelect(subdomain)}
                  >
                    <span className="session-tab-content">
                      {this.state.unseenRequests[subdomain] > 0 && (
                        <span className="unseen-count">
                          {this.state.unseenRequests[subdomain]}
                        </span>
                      )}
                      <span className="subdomain">{subdomain}</span>
                      <Button
                        icon="pi pi-times"
                        className="p-button-text p-button-secondary p-button-sm close-tab"
                        onClick={(e) => {
                          e.stopPropagation();
                          this.handleSessionRemove(subdomain);
                        }}
                        tooltip="Close Session"
                        tooltipOptions={{ position: "bottom" }}
                      />
                    </span>
                  </div>
                ),
              )}
            </div>
          )}
        </div>

        <div className="layout-topbar-icons">
          {Object.keys(this.state.sessions).length < Utils.MAX_SESSIONS && (
            <Button
              label="New Session"
              icon="pi pi-plus"
              className="p-button-text p-button-secondary new-session-button"
              onClick={this.handleNewSession}
            />
          )}

          <Button
            icon={
              "pi pi-" +
              (document.body.classList.contains("dark") ? "sun" : "moon")
            }
            className="p-button-text p-button-secondary theme-toggle"
            onClick={this.toggleTheme}
          />

          <span className="layout-topbar-search">
            <InputText
              type="text"
              placeholder="Search"
              value={this.state.searchValue}
              onChange={this.handleSearchValueChange}
            />
            <span className="layout-topbar-search-icon pi pi-search" />
          </span>
        </div>
      </div>
    );
  }
}

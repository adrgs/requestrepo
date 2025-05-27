import React, { Component } from "react";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import { Utils, Session } from "../utils";
import { toast } from "react-toastify";
import { AppSession } from "../types/app-types";

interface AppTopbarProps {
  sessions?: Record<string, AppSession>;
  activeSession?: string;
  onSessionChange?: (subdomain: string) => void;
  onSessionRemove?: (subdomain: string) => void;
  updateSearchValue?: (value: string) => void;
  onToggleMenu?: () => void;
  staticMenuInactive?: boolean;
  copyUrl?: () => void;
  copyDomain?: () => void;
  handleNewURL?: () => Promise<void>;
}

interface AppTopbarState {
  searchValue: string;
  themeToggler: boolean;
  sessions: Record<string, AppSession>;
  activeSession: string;
  unseenRequests: Record<string, number>;
  isCreatingSession: boolean;
}

export class AppTopbar extends Component<AppTopbarProps, AppTopbarState> {
  constructor(props: AppTopbarProps) {
    super(props);
    this.state = {
      searchValue: "",
      themeToggler: false,
      sessions: props.sessions || {},
      activeSession: props.activeSession || "",
      unseenRequests: {},
      isCreatingSession: false,
    };
    this.handleSearchValueChange = this.handleSearchValueChange.bind(this);
    this.toggleTheme = this.toggleTheme.bind(this);
    this.handleSessionSelect = this.handleSessionSelect.bind(this);
    this.handleSessionRemove = this.handleSessionRemove.bind(this);
    this.handleNewSession = this.handleNewSession.bind(this);
    this.handleShareSession = this.handleShareSession.bind(this);
  }

  handleShareSession(): void {
    const token = Utils.getSessionToken(this.state.activeSession);
    const url = `${window.location.origin}/?share=${token}`;
    navigator.clipboard
      .writeText(url)
      .then(() => {
        toast.success(
          "Session share link copied to clipboard",
          Utils.toastOptions,
        );
      })
      .catch(() => {
        toast.error(
          "Failed to copy session share link to clipboard",
          Utils.toastOptions,
        );
      });
  }

  toggleTheme(): void {
    Utils.toggleTheme();
    this.setState((prevState) => ({
      themeToggler: !prevState.themeToggler,
    }));
  }

  componentDidUpdate(prevProps: AppTopbarProps): void {
    const sessionsChanged = prevProps.sessions !== this.props.sessions;
    const activeSessionChanged =
      prevProps.activeSession !== this.props.activeSession;

    if (sessionsChanged || activeSessionChanged) {
      const sessions = this.props.sessions || {};
      const activeSession = this.props.activeSession || "";

      const unseenRequests = Object.keys(this.props.sessions || {}).reduce(
        (acc, subdomain) => ({
          ...acc,
          [subdomain]: this.calculateUnseenRequests(
            this.props.sessions?.[subdomain],
          ),
        }),
        {} as Record<string, number>,
      );

      this.setState({
        sessions,
        activeSession,
        unseenRequests,
      });
    }
  }

  calculateUnseenRequests(session?: AppSession): number {
    if (!session) return 0;
    const totalRequests =
      (session.httpRequests?.length || 0) + (session.dnsRequests?.length || 0);
    const visitedCount = Object.keys(session.visited || {}).length;
    return Math.max(0, totalRequests - visitedCount);
  }

  handleSessionSelect(subdomain: string): void {
    if (this.props.onSessionChange) {
      this.props.onSessionChange(subdomain);
    }
  }

  handleSessionRemove(subdomain: string): void {
    try {
      const sessionsStr = localStorage.getItem("sessions");
      if (sessionsStr) {
        const sessions = JSON.parse(sessionsStr);
        const updatedSessions = sessions.filter(
          (s: Session) => s.subdomain !== subdomain,
        );
        localStorage.setItem("sessions", JSON.stringify(updatedSessions));

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

  async handleNewSession(): Promise<void> {
    if (this.state.isCreatingSession) return;

    try {
      this.setState({ isCreatingSession: true });

      const { subdomain, token } = await Utils.getRandomSubdomain();

      const sessionsStr = localStorage.getItem("sessions");
      const sessions = JSON.parse(sessionsStr || "[]");

      const session = {
        subdomain,
        token,
        createdAt: new Date().toISOString(),
        unseenRequests: 0,
      };

      sessions.push(session);

      localStorage.setItem("sessions", JSON.stringify(sessions));

      if (this.props.onSessionChange) {
        this.props.onSessionChange(subdomain);
      }
    } catch (error) {
      console.error("Error creating new session:", error);
      throw error;
    } finally {
      this.setState({ isCreatingSession: false });
    }
  }

  handleSearchValueChange(event: React.ChangeEvent<HTMLInputElement>): void {
    this.setState({ searchValue: event.target.value });
    if (this.props.updateSearchValue) {
      this.props.updateSearchValue(event.target.value);
    }
  }

  render(): React.ReactNode {
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
            <div className="session-tabs" title="Your active sessions">
              {Object.entries(this.state.sessions).map(([subdomain]) => (
                <div
                  key={subdomain}
                  className={`session-tab ${subdomain === this.state.activeSession ? "active" : ""}`}
                  onClick={() => this.handleSessionSelect(subdomain)}
                  title={`Switch to ${subdomain} session`}
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
                      aria-label="Close session"
                    />
                  </span>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="layout-topbar-icons">
          {Object.keys(this.state.sessions).length < Utils.MAX_SESSIONS && (
            <Button
              icon="pi pi-plus"
              className="p-button-text p-button-secondary theme-toggle"
              onClick={this.handleNewSession}
              disabled={this.state.isCreatingSession}
              loading={this.state.isCreatingSession}
            />
          )}

          <Button
            icon="pi pi-share-alt"
            className="p-button-text p-button-secondary theme-toggle"
            onClick={this.handleShareSession}
            disabled={this.state.isCreatingSession}
            loading={this.state.isCreatingSession}
          />

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

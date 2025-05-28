import React, { Component } from "react";
import { RequestCard } from "./request-card";
import { Checkbox } from "primereact/checkbox";
import { Button } from "primereact/button";
import { Utils } from "../utils";
import {
  AppSession,
  HttpRequest,
  DnsRequest,
  Request,
} from "../types/app-types";

interface AppSidebarProps {
  user: AppSession | null;
  activeSession: string;
  sessions: Record<string, AppSession>;
  searchValue: string;
  deleteAllRequests: () => void;
  markAllAsVisited: () => void;
  clickRequestAction: (action: string, id: string) => void;
  updateSearchValue?: (value: string) => void;
}

interface AppSidebarState {
  http_filter: boolean;
  dns_filter: boolean;
}

export class AppSidebar extends Component<AppSidebarProps, AppSidebarState> {
  private lastNumberOfReqs: number;
  private numberOfReqs: number;
  private messagesEnd: HTMLDivElement | null;

  constructor(props: AppSidebarProps) {
    super(props);
    this.state = {
      http_filter: true,
      dns_filter: true,
    };

    this.lastNumberOfReqs = 0;
    this.numberOfReqs = 0;
    this.messagesEnd = null;
    this.onCheckboxChange = this.onCheckboxChange.bind(this);
    this.hasValue = this.hasValue.bind(this);
    this.deleteAllRequests = this.deleteAllRequests.bind(this);
  }

  scrollToBottom(): void {
    this.messagesEnd?.scrollIntoView({ behavior: "auto" });
  }

  componentDidMount(): void {
    this.scrollToBottom();
  }

  componentDidUpdate(prevProps: AppSidebarProps): void {
    if (this.numberOfReqs > this.lastNumberOfReqs) {
      this.scrollToBottom();
    }
    this.lastNumberOfReqs = this.numberOfReqs;
    if (prevProps.activeSession !== this.props.activeSession) {
      this.setState({ http_filter: true, dns_filter: true });
    }
  }

  shouldComponentUpdate(
    nextProps: AppSidebarProps,
    nextState: AppSidebarState,
  ): boolean {
    return (
      nextProps.user !== this.props.user ||
      nextProps.activeSession !== this.props.activeSession ||
      nextProps.sessions !== this.props.sessions ||
      nextProps.searchValue !== this.props.searchValue ||
      this.state.http_filter !== nextState.http_filter ||
      this.state.dns_filter !== nextState.dns_filter
    );
  }

  onCheckboxChange(event: { value: string }): void {
    const { value } = event;
    this.setState((prevState) => {
      if (value === "HTTP") {
        return { ...prevState, http_filter: !prevState.http_filter };
      } else if (value === "DNS") {
        return { ...prevState, dns_filter: !prevState.dns_filter };
      }

      return prevState;
    });
  }

  convertUTCDateToLocalDate(date: number): Date {
    const utcSeconds = date;
    const d = new Date(0);
    d.setUTCSeconds(utcSeconds);
    return d;
  }

  getRequests(): Array<{
    title: string;
    method: string;
    time: string;
    detail: string;
    country?: string;
    id: string;
    key: string;
    type: string;
    new?: boolean;
  }> {
    const requests: Array<{
      title: string;
      method: string;
      time: string;
      detail: string;
      country?: string;
      id: string;
      key: string;
      type: string;
      new?: boolean;
    }> = [];

    if (!this.props.user) {
      return requests;
    }

    const session = this.props.user;
    if (!session || !session.requests) {
      return requests;
    }

    if (session.httpRequests && session.dnsRequests) {
      let i = 0,
        j = 0;
      while (
        i < session.httpRequests.length ||
        j < session.dnsRequests.length
      ) {
        const obj: {
          title: string | null;
          method: string | null;
          time: string | null;
          detail: string | null;
          country?: string;
          id: string | null;
          key?: string;
          type: string | null;
          new?: boolean;
        } = {
          title: null,
          method: null,
          time: null,
          detail: null,
          id: null,
          type: null,
        };

        let dateA = 0;
        let dateB = 0;
        if (i < session.httpRequests.length) {
          dateA = parseInt(String(session.httpRequests[i].date));
        }
        if (j < session.dnsRequests.length) {
          dateB = parseInt(String(session.dnsRequests[j].date));
        }

        if (
          (j >= session.dnsRequests.length || dateA < dateB) &&
          i < session.httpRequests.length
        ) {
          const req = ((session.requests &&
            session.requests[session.httpRequests[i]["_id"]]) ||
            {}) as HttpRequest;

          obj["title"] =
            (req.path || "") +
            (req.query ? req.query : "") +
            (req.fragment ? req.fragment : "");
          obj["method"] = req.method;
          obj["time"] = this.convertUTCDateToLocalDate(dateA).toLocaleString();
          obj["detail"] = req.ip;
          obj["country"] = req.country || "";
          obj["id"] = req._id;
          obj["key"] = obj["id"] as string;
          obj["type"] = "HTTP";
          obj["new"] = req.new || false;

          requests.push(
            obj as {
              title: string;
              method: string;
              time: string;
              detail: string;
              country?: string;
              id: string;
              key: string;
              type: string;
              new?: boolean;
            },
          );
          i++;
        } else {
          const req = ((session.requests &&
            session.requests[session.dnsRequests[j]["_id"]]) ||
            {}) as DnsRequest;

          obj["title"] = req.query || "";
          obj["method"] = "DNS";
          obj["time"] = this.convertUTCDateToLocalDate(dateB).toLocaleString();
          obj["detail"] = req.ip || "";
          obj["country"] = req.country || "";
          obj["id"] = req._id;
          obj["key"] = obj["id"] as string;
          obj["type"] = "DNS";
          obj["new"] = req.new || false;

          requests.push(
            obj as {
              title: string;
              method: string;
              time: string;
              detail: string;
              country?: string;
              id: string;
              key: string;
              type: string;
              new?: boolean;
            },
          );

          j++;
        }
      }
    }
    this.numberOfReqs = requests.length;

    return requests;
  }

  deleteAllRequests(): void {
    if (!this.props.user) {
      return;
    }

    this.props.deleteAllRequests();
  }

  hasValue(
    item: Request | Record<string, string | number | boolean | null | object>,
    needle: string,
  ): boolean {
    if (!needle) return true;
    if ("name" in item && item.name !== undefined) {
      if ("dns".indexOf(needle) >= 0) return true;
    } else {
      if ("http".indexOf(needle) >= 0) return true;
    }
    needle = needle.toLowerCase();
    for (const property in item) {
      const val = item[property as keyof typeof item];
      if (property === "raw" && typeof val === "string") {
        const decodedVal = Utils.base64Decode(val).toString().toLowerCase();
        if (decodedVal.indexOf(needle) >= 0) return true;
        continue;
      }
      if (
        property === "date" &&
        (typeof val === "string" || typeof val === "number")
      ) {
        const dateVal = this.convertUTCDateToLocalDate(parseInt(String(val)))
          .toLocaleString()
          .toLowerCase();
        if (dateVal.indexOf(needle) >= 0) return true;
        continue;
      }

      if (typeof val === "object" && val !== null) {
        const objVal = val as Record<
          string,
          string | number | boolean | null | object
        >;
        for (const prop in objVal) {
          if (Object.prototype.hasOwnProperty.call(objVal, prop)) {
            const propValue = objVal[prop];
            if (propValue !== null && propValue !== undefined) {
              const val2 = String(propValue).toLowerCase();
              const val3 = String(prop).toLowerCase();
              if (val2.indexOf(needle) >= 0) return true;
              if (val3.indexOf(needle) >= 0) return true;
            }
          }
        }
      } else if (val !== null && val !== undefined) {
        const strVal = String(val).toLowerCase();
        if (strVal.indexOf(needle) >= 0) return true;
      }
    }

    return false;
  }

  render(): React.ReactNode {
    const hasValue = this.hasValue;

    let requestsList = this.getRequests();

    const searchValue = this.props.searchValue;
    const dns_filter = this.state.dns_filter;
    const http_filter = this.state.http_filter;
    requestsList = requestsList.filter(
      function (this: AppSidebar, item: { id: string; type: string }) {
        return (
          hasValue(
            this.props.user?.requests && this.props.user.requests[item.id]
              ? this.props.user.requests[item.id]
              : {},
            searchValue,
          ) &&
          ((item.type === "DNS" && dns_filter) ||
            (item.type === "HTTP" && http_filter))
        );
      }.bind(this),
    );
    return (
      <div className={"layout-sidebar layout-sidebar-light"}>
        <div className={"layout-sidebar-header"}>
          <div className={"layout-sidebar-subheader"}>
            <Button
              label="Delete all requests"
              icon="pi pi-times"
              className="p-button-danger p-button-text"
              onClick={this.deleteAllRequests}
            />
          </div>
          <div style={{ padding: "0.85rem" }}>
            <b style={{ marginRight: "20px" }}>
              Requests ({requestsList.length})
            </b>
            <Checkbox
              value="HTTP"
              inputId="cbHTTP"
              onChange={this.onCheckboxChange}
              checked={this.state.http_filter}
            />
            <label
              style={{ marginRight: "15px" }}
              htmlFor="cbHTTP"
              className="p-checkbox-label"
            >
              HTTP
            </label>
            <Checkbox
              value="DNS"
              inputId="cbDNS"
              onChange={this.onCheckboxChange}
              checked={this.state.dns_filter}
            />
            <label htmlFor="cbDNS" className="p-checkbox-label">
              DNS
            </label>
          </div>
        </div>
        <div className="requests-box">
          {requestsList.map(
            (item: {
              title: string;
              method: string;
              time: string;
              detail: string;
              country?: string;
              id: string;
              key: string;
              type: string;
              new?: boolean;
            }) => {
              return (
                <RequestCard
                  active={this.props.user?.selectedRequest === item.id}
                  visited={this.props.user?.visited?.[item.id] !== undefined}
                  title={item.title}
                  time={item.time}
                  new={item.new}
                  method={item.method}
                  country={item.country}
                  detail={item.detail}
                  id={item.id}
                  key={item.key}
                  clickRequestAction={this.props.clickRequestAction}
                  sessionId={this.props.user?.subdomain}
                />
              );
            },
          )}
          <div
            style={{ float: "left", clear: "both" }}
            ref={(el) => {
              this.messagesEnd = el;
            }}
          ></div>
        </div>
        <div
          className="github-button"
          style={{
            position: "absolute",
            bottom: "0",
            height: "100px",
            textAlign: "center",
            width: "100%",
          }}
        >
          <Button
            label="Mark all as read"
            icon="pi pi-check-square"
            className="p-button-text p-button-secondary"
            style={{ marginRight: ".25em" }}
            onClick={() => this.props.user && this.props.markAllAsVisited()}
          />
        </div>
      </div>
    );
  }
}

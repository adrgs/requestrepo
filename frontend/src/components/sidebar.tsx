import React, { Component } from "react";
import { RequestCard } from "./request-card";
import { Checkbox } from "primereact/checkbox";
import { Button } from "primereact/button";
import { Utils } from "../utils";

interface AppSession {
  url: string;
  domain: string;
  subdomain: string;
  httpRequests: any[];
  dnsRequests: any[];
  timestamp: string | null;
  requests: Record<string, any>;
  visited: Record<string, boolean>;
  selectedRequest: string | null;
  token: string;
}

interface AppSidebarProps {
  user: AppSession | null;
  activeSession: string;
  sessions: Record<string, AppSession>;
  searchValue: string;
  deleteAllRequests: () => void;
  markAllAsVisited: () => void;
  clickRequestAction: (id: string, action: string) => void;
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
    var utcSeconds = date;
    var d = new Date(0);
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
    let requests: Array<{
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
        let obj: {
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
          dateA = parseInt(session.httpRequests[i].date);
        }
        if (j < session.dnsRequests.length) {
          dateB = parseInt(session.dnsRequests[j].date);
        }

        if (
          (j >= session.dnsRequests.length || dateA < dateB) &&
          i < session.httpRequests.length
        ) {
          let req =
            (session.requests &&
              session.requests[session.httpRequests[i]["_id"]]) ||
            {};
          obj["title"] =
            req["path"] +
            (req["query"] ? req["query"] : "") +
            (req["fragment"] ? req["fragment"] : "");
          obj["method"] = req["method"];
          obj["time"] = this.convertUTCDateToLocalDate(dateA).toLocaleString();
          obj["detail"] = req["ip"];
          obj["country"] = req["country"];
          obj["id"] = req["_id"];
          obj["key"] = obj["id"] as string;
          obj["type"] = "HTTP";
          obj["new"] = req["new"];

          requests.push(obj as any);
          i++;
        } else {
          let req =
            (session.requests &&
              session.requests[session.dnsRequests[j]["_id"]]) ||
            {};
          obj["title"] = req["name"];
          obj["method"] = "DNS";
          obj["time"] = this.convertUTCDateToLocalDate(dateB).toLocaleString();
          obj["detail"] = req["ip"];
          obj["country"] = req["country"];
          obj["id"] = req["_id"];
          obj["key"] = obj["id"] as string;
          obj["type"] = "DNS";
          obj["new"] = req["new"];

          requests.push(obj as any);

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

  hasValue(item: any, needle: string): boolean {
    if (!needle) return true;
    if (item["name"] !== undefined) {
      if ("dns".indexOf(needle) >= 0) return true;
    } else {
      if ("http".indexOf(needle) >= 0) return true;
    }
    needle = needle.toLowerCase();
    for (let property in item) {
      let val = item[property];
      if (property === "raw") {
        val = Utils.base64Decode(item[property]).toString().toLowerCase();
        if (val.indexOf(needle) >= 0) return true;
        continue;
      }
      if (property === "date") {
        val = this.convertUTCDateToLocalDate(parseInt(val))
          .toLocaleString()
          .toLowerCase();
        if (val.indexOf(needle) >= 0) return true;
        continue;
      }

      if (typeof val === "object") {
        for (let prop in val) {
          let val2 = val[prop].toString().toLowerCase();
          let val3 = prop.toString().toLowerCase();
          if (val2.indexOf(needle) >= 0) return true;
          if (val3.indexOf(needle) >= 0) return true;
        }
      } else {
        val = val.toString().toLowerCase();
        if (val.indexOf(needle) >= 0) return true;
      }
    }

    return false;
  }

  render(): React.ReactNode {
    const hasValue = this.hasValue;

    let requests = this.getRequests();

    let searchValue = this.props.searchValue;
    let dns_filter = this.state.dns_filter;
    let http_filter = this.state.http_filter;
    requests = requests.filter(
      function (this: AppSidebar, item: { id: string; type: string }) {
        return (
          hasValue(
            (this.props.user?.requests && this.props.user.requests[item.id]) ||
              {},
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
            <b style={{ marginRight: "20px" }}>Requests ({requests.length})</b>
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
          {requests.map((item) => {
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
          })}
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

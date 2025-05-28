import React, { useState, useEffect, useRef, useCallback } from "react";
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

export function AppSidebar({
  user,
  activeSession,
  sessions,
  searchValue,
  deleteAllRequests,
  markAllAsVisited,
  clickRequestAction,
}: AppSidebarProps): React.ReactElement {
  const [httpFilter, setHttpFilter] = useState<boolean>(true);
  const [dnsFilter, setDnsFilter] = useState<boolean>(true);
  const lastNumberOfReqsRef = useRef<number>(0);
  const numberOfReqsRef = useRef<number>(0);
  const messagesEndRef = useRef<HTMLDivElement | null>(null);

  const scrollToBottom = useCallback((): void => {
    messagesEndRef.current?.scrollIntoView({ behavior: "auto" });
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [scrollToBottom]);

  useEffect(() => {
    if (numberOfReqsRef.current > lastNumberOfReqsRef.current) {
      scrollToBottom();
    }
    lastNumberOfReqsRef.current = numberOfReqsRef.current;
  }, [user, scrollToBottom]);

  useEffect(() => {
    if (activeSession !== activeSession) {
      setHttpFilter(true);
      setDnsFilter(true);
    }
  }, [activeSession]);

  const onCheckboxChange = useCallback((event: { value: string }): void => {
    const { value } = event;
    if (value === "HTTP") {
      setHttpFilter((prev) => !prev);
    } else if (value === "DNS") {
      setDnsFilter((prev) => !prev);
    }
  }, []);

  const convertUTCDateToLocalDate = useCallback((date: number): Date => {
    const utcSeconds = date;
    const d = new Date(0);
    d.setUTCSeconds(utcSeconds);
    return d;
  }, []);

  const getRequests = useCallback((): Array<{
    title: string;
    method: string;
    time: string;
    detail: string;
    country?: string;
    id: string;
    key: string;
    type: string;
    new?: boolean;
  }> => {
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

    if (!user) {
      return requests;
    }

    const session = user;
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
          obj["time"] = convertUTCDateToLocalDate(dateA).toLocaleString();
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
          obj["time"] = convertUTCDateToLocalDate(dateB).toLocaleString();
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
    numberOfReqsRef.current = requests.length;

    return requests;
  }, [user, convertUTCDateToLocalDate]);

  const handleDeleteAllRequests = useCallback((): void => {
    if (!user) {
      return;
    }

    deleteAllRequests();
  }, [user, deleteAllRequests]);

  const hasValue = useCallback(
    (
      item: Request | Record<string, string | number | boolean | null | object>,
      needle: string,
    ): boolean => {
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
          const dateVal = convertUTCDateToLocalDate(parseInt(String(val)))
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
    },
    [convertUTCDateToLocalDate],
  );

  let requestsList = getRequests();

  requestsList = requestsList.filter((item) => {
    return (
      hasValue(
        user?.requests && user.requests[item.id] ? user.requests[item.id] : {},
        searchValue,
      ) &&
      ((item.type === "DNS" && dnsFilter) ||
        (item.type === "HTTP" && httpFilter))
    );
  });

  return (
    <div className={"layout-sidebar layout-sidebar-light"}>
      <div className={"layout-sidebar-header"}>
        <div className={"layout-sidebar-subheader"}>
          <Button
            label="Delete all requests"
            icon="pi pi-times"
            className="p-button-danger p-button-text"
            onClick={handleDeleteAllRequests}
          />
        </div>
        <div style={{ padding: "0.75rem" }}>
          <b style={{ marginRight: "16px", fontSize: "0.875rem" }}>
            Requests ({requestsList.length})
          </b>
          <Checkbox
            value="HTTP"
            inputId="cbHTTP"
            onChange={onCheckboxChange}
            checked={httpFilter}
          />
          <label
            style={{ marginRight: "12px", fontSize: "0.875rem" }}
            htmlFor="cbHTTP"
            className="p-checkbox-label"
          >
            HTTP
          </label>
          <Checkbox
            value="DNS"
            inputId="cbDNS"
            onChange={onCheckboxChange}
            checked={dnsFilter}
          />
          <label
            htmlFor="cbDNS"
            className="p-checkbox-label"
            style={{ fontSize: "0.875rem" }}
          >
            DNS
          </label>
        </div>
      </div>
      <div className="requests-box">
        {requestsList.map((item) => {
          return (
            <RequestCard
              active={user?.selectedRequest === item.id}
              visited={user?.visited?.[item.id] !== undefined}
              title={item.title}
              time={item.time}
              new={item.new}
              method={item.method}
              country={item.country}
              detail={item.detail}
              id={item.id}
              key={item.key}
              clickRequestAction={clickRequestAction}
              sessionId={user?.subdomain}
            />
          );
        })}
        <div
          style={{ float: "left", clear: "both" }}
          ref={messagesEndRef}
        ></div>
      </div>
      <div
        className="github-button"
        style={{
          position: "absolute",
          bottom: "0",
          height: "80px" /* Reduced height for more compact design */,
          textAlign: "center",
          width: "100%",
        }}
      >
        <Button
          label="Mark all as read"
          icon="pi pi-check-square"
          className="p-button-text p-button-secondary"
          style={{ marginRight: ".25em", fontSize: "0.875rem" }}
          onClick={() => user && markAllAsVisited()}
        />
      </div>
    </div>
  );
}

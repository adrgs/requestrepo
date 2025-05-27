import { useState, useCallback } from "react";
import { AppSession, Request, HttpRequest, DnsRequest } from "../types/app-types";
import { Utils } from "../utils";

export function useRequestHandler(
  initialSessions: Record<string, AppSession>,
  initialActiveSession: string
): {
  sessions: Record<string, AppSession>;
  activeSession: string;
  handleMessage: (event: MessageEvent, subdomain: string) => void;
  clickRequestAction: (action: string, id: string) => void;
  markAllAsVisited: () => void;
  deleteAllRequests: () => void;
  updateDocumentTitle: () => void;
} {
  const [sessions, setSessions] = useState<Record<string, AppSession>>(initialSessions);
  const [activeSession, setActiveSession] = useState<string>(initialActiveSession);

  const updateDocumentTitle = useCallback(() => {
    let totalUnseen = 0;
    Object.values(sessions).forEach((session) => {
      let unseenCount = 0;
      Object.keys(session.requests).forEach((key) => {
        if (session.requests[key].new && !session.visited[key]) {
          unseenCount++;
        }
      });
      totalUnseen += unseenCount;
    });

    document.title =
      totalUnseen > 0 ? `(${totalUnseen}) RequestRepo` : "RequestRepo";
  }, [sessions]);

  const handleMessage = useCallback(
    (event: MessageEvent, subdomain: string) => {
      const data = JSON.parse(event.data as string);
      const { cmd } = data;

      if (!subdomain || subdomain === "") {
        console.warn(
          "Received WebSocket message with empty subdomain, ignoring"
        );
        return;
      }

      setSessions((prevSessions) => {
        const newSessions = { ...prevSessions };

        const existingSession = newSessions[subdomain];
        const session = existingSession || {
          url: `${subdomain}.${Utils.siteUrl}`,
          domain: Utils.siteUrl,
          subdomain: subdomain,
          httpRequests: [],
          dnsRequests: [],
          timestamp: null,
          requests: {},
          visited: {},
          selectedRequest: null,
          token: "",
          dnsRecords: [],
        };

        if (cmd === "invalid_token") {
          const token = data["token"];
          if (token) {
            try {
              const tokenSubdomain = JSON.parse(atob(token.split(".")[1]))[
                "subdomain"
              ];
              console.error(`Invalid token for ${tokenSubdomain}`);
            } catch (err) {
              console.error("Error parsing token:", err);
            }
          }
        } else if (cmd === "requests") {
          const requests = data["data"].map(JSON.parse);
          requests.forEach((request: Request) => {
            const key = request["_id"];

            if (!session.requests[key]) {
              session.requests[key] = request;

              if (request["type"] === "http") {
                if (!session.httpRequests.some((r) => r["_id"] === key)) {
                  session.httpRequests.push(request as HttpRequest);
                }
              } else if (request["type"] === "dns") {
                if (!session.dnsRequests.some((r) => r["_id"] === key)) {
                  session.dnsRequests.push(request as DnsRequest);
                }
              }
            }
          });
        } else if (cmd === "request") {
          const request = JSON.parse(data["data"]) as Request;
          const key = request["_id"];

          if (!session.requests[key]) {
            request["new"] = true;
            session.requests[key] = request;

            if (request["type"] === "http") {
              if (!session.httpRequests.some((r) => r["_id"] === key)) {
                session.httpRequests.push(request as HttpRequest);
              }
            } else if (request["type"] === "dns") {
              if (!session.dnsRequests.some((r) => r["_id"] === key)) {
                session.dnsRequests.push(request as DnsRequest);
              }
            }
          }
        } else if (cmd === "dns_records") {
          session.dnsRecords = data.records;
        }

        if (subdomain && subdomain !== "") {
          newSessions[subdomain] = session;
        }

        return newSessions;
      });

      setTimeout(updateDocumentTitle, 0);
    },
    [updateDocumentTitle]
  );

  const markAllAsVisited = useCallback(() => {
    if (!activeSession) return;

    setSessions((prevSessions) => {
      const session = prevSessions[activeSession];
      if (!session) return prevSessions;

      const newVisited = { ...session.visited };
      Object.keys(session.requests).forEach((key) => {
        newVisited[key] = true;
        session.requests[key].new = false;
      });

      const newSessions = { ...prevSessions };
      newSessions[activeSession] = {
        ...session,
        visited: newVisited,
      };

      return newSessions;
    });

    updateDocumentTitle();
  }, [activeSession, updateDocumentTitle]);

  const clickRequestAction = useCallback(
    (action: string, id: string) => {
      if (!activeSession) return;

      setSessions((prevSessions) => {
        const session = prevSessions[activeSession];
        if (!session) return prevSessions;

        if (action === "select") {
          const newVisited = { ...session.visited };
          newVisited[id] = true;
          if (session.requests[id]) {
            session.requests[id].new = false;
          }

          const newSessions = { ...prevSessions };
          newSessions[activeSession] = {
            ...session,
            visited: newVisited,
            selectedRequest: id,
          };

          updateDocumentTitle();
          return newSessions;
        } else if (action === "delete") {
          const newRequests = { ...session.requests };
          delete newRequests[id];

          const combinedRequests = [
            ...session.httpRequests,
            ...session.dnsRequests,
          ];
          const deleteIndex = combinedRequests.findIndex((r) => r._id === id);
          const nextSelectedIndex = Math.min(
            deleteIndex + 1,
            combinedRequests.length - 1
          );
          const nextSelectedIndex2 = Math.max(0, nextSelectedIndex);
          const nextSelectedId =
            nextSelectedIndex2 >= 0 &&
            nextSelectedIndex2 < combinedRequests.length
              ? combinedRequests[nextSelectedIndex2]._id
              : null;

          const newHttpRequests = session.httpRequests.filter(
            (r) => r._id !== id
          );
          const newDnsRequests = session.dnsRequests.filter(
            (r) => r._id !== id
          );

          const newSessions = { ...prevSessions };
          newSessions[activeSession] = {
            ...session,
            requests: newRequests,
            httpRequests: newHttpRequests,
            dnsRequests: newDnsRequests,
            selectedRequest: nextSelectedId,
          };

          Utils.deleteRequest(id, session.subdomain);

          return newSessions;
        }

        return prevSessions;
      });
    },
    [activeSession, updateDocumentTitle]
  );

  const deleteAllRequests = useCallback(() => {
    if (!activeSession) return;

    setSessions((prevSessions) => {
      const session = prevSessions[activeSession];
      if (!session) return prevSessions;

      const newSessions = { ...prevSessions };
      newSessions[activeSession] = {
        ...session,
        httpRequests: [],
        dnsRequests: [],
        requests: {},
        selectedRequest: null,
      };

      return newSessions;
    });

    Utils.deleteAll(activeSession);
  }, [activeSession]);

  return {
    sessions,
    activeSession,
    handleMessage,
    clickRequestAction,
    markAllAsVisited,
    deleteAllRequests,
    updateDocumentTitle,
  };
}

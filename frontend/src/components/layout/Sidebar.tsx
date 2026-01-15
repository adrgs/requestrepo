import { useMemo, useRef, useEffect } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { Button, Checkbox, ScrollShadow } from "@heroui/react";
import { X, CheckCheck } from "lucide-react";
import { toast } from "sonner";
import { useSessionStore } from "@/stores/sessionStore";
import {
  useRequestStore,
  broadcastDeleteRequest,
  broadcastClearRequests,
} from "@/stores/requestStore";
import { useUiStore } from "@/stores/uiStore";
import { apiClient } from "@/api/client";
import { cn, formatRelativeTime, getFlagClass } from "@/lib/utils";
import type { Request } from "@/types";
import { isHttpRequest, isSmtpRequest } from "@/types";

export function Sidebar() {
  const navigate = useNavigate();
  const location = useLocation();

  const sessions = useSessionStore((s) => s.sessions);
  const activeSubdomain = useSessionStore((s) => s.activeSubdomain);
  const session = sessions.find((s) => s.subdomain === activeSubdomain);

  const allRequests = useRequestStore((s) => s.requests);
  const requests = useMemo(
    () => (activeSubdomain ? (allRequests[activeSubdomain] ?? []) : []),
    [activeSubdomain, allRequests],
  );
  const clearRequests = useRequestStore((s) => s.clearRequests);

  const httpFilter = useUiStore((s) => s.httpFilter);
  const dnsFilter = useUiStore((s) => s.dnsFilter);
  const smtpFilter = useUiStore((s) => s.smtpFilter);
  const searchQuery = useUiStore((s) => s.searchQuery);
  const setFilters = useUiStore((s) => s.setFilters);
  const selectedRequestId = useUiStore((s) => s.selectedRequestId);
  const selectRequest = useUiStore((s) => s.selectRequest);
  const visitedRequests = useUiStore((s) => s.visitedRequests);
  const markRequestVisited = useUiStore((s) => s.markRequestVisited);
  const markAllAsRead = useUiStore((s) => s.markAllAsRead);

  const removeRequest = useRequestStore((s) => s.removeRequest);

  // Refs for scroll behavior
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const requestRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const prevRequestCountRef = useRef<number>(0);
  const initialScrollDoneRef = useRef<boolean>(false);

  const filteredRequests = useMemo(() => {
    return requests
      .filter((r) => {
        // Type filter
        if (r.type === "http" && !httpFilter) return false;
        if (r.type === "dns" && !dnsFilter) return false;
        if (r.type === "smtp" && !smtpFilter) return false;

        // Search filter
        if (searchQuery) {
          const query = searchQuery.toLowerCase();
          if (isHttpRequest(r)) {
            return (
              r.path?.toLowerCase().includes(query) ||
              r.ip?.toLowerCase().includes(query) ||
              r.method?.toLowerCase().includes(query) ||
              r.url?.toLowerCase().includes(query)
            );
          } else if (isSmtpRequest(r)) {
            return (
              r.command?.toLowerCase().includes(query) ||
              r.data?.toLowerCase().includes(query) ||
              r.ip?.toLowerCase().includes(query)
            );
          } else {
            // DNS request
            return (
              r.domain?.toLowerCase().includes(query) ||
              r.ip?.toLowerCase().includes(query) ||
              r.query_type?.toLowerCase().includes(query)
            );
          }
        }
        return true;
      })
      .sort((a, b) => a.date - b.date); // oldest first, newest at bottom
  }, [requests, httpFilter, dnsFilter, smtpFilter, searchQuery]);

  // Scroll to bottom helper
  const scrollToBottom = () => {
    bottomRef.current?.scrollIntoView({ behavior: "auto" });
  };

  // Scroll to specific request (centered)
  const scrollToRequest = (requestId: string) => {
    const element = requestRefs.current.get(requestId);
    if (element) {
      element.scrollIntoView({ behavior: "auto", block: "center" });
    }
  };

  // Initial scroll on mount - scroll to selected request or bottom
  useEffect(() => {
    if (initialScrollDoneRef.current || filteredRequests.length === 0) return;

    // Small delay to ensure DOM is ready
    const timer = setTimeout(() => {
      if (selectedRequestId && requestRefs.current.has(selectedRequestId)) {
        scrollToRequest(selectedRequestId);
      } else {
        scrollToBottom();
      }
      initialScrollDoneRef.current = true;
      prevRequestCountRef.current = filteredRequests.length;
    }, 100);

    return () => clearTimeout(timer);
  }, [filteredRequests.length, selectedRequestId]);

  // Scroll to bottom when new requests arrive
  useEffect(() => {
    if (!initialScrollDoneRef.current) return;

    if (filteredRequests.length > prevRequestCountRef.current) {
      scrollToBottom();
    }
    prevRequestCountRef.current = filteredRequests.length;
  }, [filteredRequests.length]);

  // Reset initial scroll flag when subdomain changes
  useEffect(() => {
    initialScrollDoneRef.current = false;
    prevRequestCountRef.current = 0;
  }, [activeSubdomain]);

  const handleDeleteRequest = async (
    e: React.MouseEvent,
    requestId: string,
  ) => {
    e.stopPropagation();
    if (!session) return;
    try {
      await apiClient.deleteRequest(session.token, requestId);
      removeRequest(activeSubdomain!, requestId);
      broadcastDeleteRequest(activeSubdomain!, requestId); // Notify other tabs
      if (selectedRequestId === requestId) {
        selectRequest(null);
      }
    } catch (error) {
      console.error("Delete request failed:", error);
      toast.error("Failed to delete request");
    }
  };

  // Check if all are read
  const allRead = useMemo(() => {
    if (!activeSubdomain || requests.length === 0) return true;
    return requests.every((r) => visitedRequests[activeSubdomain]?.[r._id]);
  }, [activeSubdomain, requests, visitedRequests]);

  const handleDeleteAll = async () => {
    if (!session) return;
    try {
      await apiClient.deleteAllRequests(session.token);
      clearRequests(activeSubdomain!);
      broadcastClearRequests(activeSubdomain!); // Notify other tabs
      selectRequest(null);
      toast.success("All requests deleted");
    } catch (error) {
      console.error("Delete all requests failed:", error);
      toast.error("Failed to delete requests");
    }
  };

  const handleMarkAllRead = () => {
    if (!activeSubdomain || allRead) return;
    markAllAsRead(
      activeSubdomain,
      requests.map((r) => r._id),
    );
  };

  const handleSelectRequest = (request: Request) => {
    selectRequest(request._id);
    if (activeSubdomain) {
      markRequestVisited(activeSubdomain, request._id);
    }
    // Navigate to requests page if not already there
    if (location.pathname !== "/requests") {
      navigate("/requests");
    }
  };

  return (
    <div className="flex h-full flex-col">
      {/* Delete all requests button - aligned with toolbar (h-12 = 48px) */}
      <div className="flex h-12 shrink-0 items-center px-3">
        <Button
          color="danger"
          variant="flat"
          size="sm"
          className="w-full"
          startContent={<X className="h-4 w-4" />}
          onPress={handleDeleteAll}
          isDisabled={requests.length === 0}
          radius="lg"
        >
          Delete all requests
        </Button>
      </div>

      {/* Requests header + filters */}
      <div className="mb-3 px-3">
        <p className="mb-2 text-sm font-medium text-default-500">
          Requests ({requests.length})
        </p>
        <div className="flex items-center gap-3">
          <Checkbox
            isSelected={httpFilter}
            onValueChange={(checked) => setFilters(checked, dnsFilter, smtpFilter)}
            size="sm"
            radius="full"
          >
            <span className="text-sm">HTTP</span>
          </Checkbox>
          <Checkbox
            isSelected={dnsFilter}
            onValueChange={(checked) => setFilters(httpFilter, checked, smtpFilter)}
            size="sm"
            radius="full"
          >
            <span className="text-sm">DNS</span>
          </Checkbox>
          <Checkbox
            isSelected={smtpFilter}
            onValueChange={(checked) => setFilters(httpFilter, dnsFilter, checked)}
            size="sm"
            radius="full"
          >
            <span className="text-sm">SMTP</span>
          </Checkbox>
        </div>
      </div>

      {/* Request list */}
      <ScrollShadow
        ref={scrollContainerRef}
        className="flex-1 overflow-auto px-2"
      >
        <div className="flex flex-col">
          {filteredRequests.map((request, index) => {
            const isActive = request._id === selectedRequestId;
            const isVisited = activeSubdomain
              ? (visitedRequests[activeSubdomain]?.[request._id] ?? false)
              : false;
            const isHttp = isHttpRequest(request);
            const isSmtp = isSmtpRequest(request);
            const isFirst = index === 0;
            const isLast = index === filteredRequests.length - 1;

            // Determine badge color
            const getBadgeColor = () => {
              if (isSmtp) return "bg-[#e91e63]"; // Pink for SMTP
              if (!isHttp) return "bg-[#33daff]"; // Cyan for DNS
              // HTTP methods
              switch (request.method) {
                case "GET":
                  return "bg-[#20d077]";
                case "POST":
                  return "bg-[#ffae00]";
                case "PUT":
                  return "bg-[#ff9800]";
                case "DELETE":
                  return "bg-[#f44336]";
                default:
                  return "bg-[#9e9e9e]";
              }
            };

            // Determine badge text
            const getBadgeText = () => {
              if (isHttp) return request.method;
              if (isSmtp) return "SMTP";
              return "DNS";
            };

            // Determine display text
            const getDisplayText = () => {
              if (isHttp) {
                return `${request.path}${request.query ? `?${request.query}` : ""}${request.fragment ? `#${request.fragment}` : ""}`;
              }
              if (isSmtp) {
                return request.command;
              }
              return request.domain;
            };

            return (
              <div
                key={request._id}
                ref={(el) => {
                  if (el) requestRefs.current.set(request._id, el);
                  else requestRefs.current.delete(request._id);
                }}
                onClick={() => handleSelectRequest(request)}
                className={cn(
                  "group cursor-pointer px-2 py-1.5 transition-colors hover:bg-default-200 dark:hover:bg-default-100",
                  isFirst && "rounded-t-md",
                  isLast && "rounded-b-md",
                  isActive && "bg-primary/10",
                  isVisited && !isActive && "bg-default-100 dark:bg-black/20",
                )}
              >
                <div className="flex items-center gap-1.5">
                  {!isVisited && (
                    <span className="shrink-0 inline-block px-1 py-px text-[8px] font-semibold text-white bg-red-500 rounded">
                      NEW
                    </span>
                  )}
                  <span
                    className={`shrink-0 inline-block px-1 py-px text-[8px] font-semibold text-white rounded ${getBadgeColor()}`}
                  >
                    {getBadgeText()}
                  </span>
                  <span
                    className={cn(
                      "truncate text-xs",
                      !isVisited && "font-medium",
                    )}
                  >
                    {getDisplayText()}
                  </span>
                  <span
                    className="ml-auto shrink-0 px-1 py-px text-[8px] font-semibold text-white bg-red-500 hover:bg-red-600 rounded cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity"
                    onClick={(e) => handleDeleteRequest(e, request._id)}
                  >
                    X
                  </span>
                </div>
                <div className="mt-1.5 flex items-center gap-1 text-[10px] text-default-400">
                  <span className={getFlagClass(request.country)} />
                  <span className="truncate">{request.ip}</span>
                  <span className="ml-auto shrink-0">
                    {formatRelativeTime(request.date)}
                  </span>
                </div>
              </div>
            );
          })}
          {/* Scroll anchor for auto-scroll to bottom */}
          <div ref={bottomRef} />
        </div>
      </ScrollShadow>

      {/* Mark all as read */}
      <div className="shrink-0 p-3">
        <Button
          color="default"
          variant="flat"
          size="sm"
          className="w-full"
          startContent={<CheckCheck className="h-4 w-4" />}
          onPress={handleMarkAllRead}
          isDisabled={requests.length === 0 || allRead}
          radius="lg"
        >
          Mark all as read
        </Button>
      </div>
    </div>
  );
}

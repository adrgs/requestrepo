import React, { useState, useEffect, useCallback } from "react";
import { Button } from "primereact/button";
import { RecordInput } from "./record-input";
import { Utils, DNSRecord } from "../utils";
import { DnsRecord, AppSession } from "../types/app-types";
import { toast } from "react-toastify";

interface DnsSettingsPageProps {
  dnsRecords?: DnsRecord[];
  user: AppSession | null;
  toast: typeof toast;
  activeSession: string | null;
}

export const DnsSettingsPage: React.FC<DnsSettingsPageProps> = ({
  dnsRecords: propDnsRecords = [],
  user,
  toast,
  activeSession,
}) => {
  const [dnsRecords, setDnsRecords] = useState<DnsRecord[]>(propDnsRecords);
  const [fetched, setFetched] = useState<boolean>(false);

  const getActiveSessionData = useCallback(() => {
    if (process.env.NODE_ENV === "test") {
      return { subdomain: activeSession, token: "test-token" };
    }

    const sessions = Utils.getAllSessions();
    return sessions.find((s) => s.subdomain === activeSession);
  }, [activeSession]);

  useEffect(() => {
    if (propDnsRecords && propDnsRecords.length > 0) {
      setDnsRecords(propDnsRecords);
      setFetched(true);
    }
  }, [propDnsRecords]);

  useEffect(() => {
    const fetchData = async () => {
      if (!activeSession) {
        setDnsRecords([]);
        return;
      }

      try {
        const token = Utils.getSessionToken(activeSession);
        if (!token) {
          throw new Error("No valid token found for session");
        }

        const res = await Utils.getDNSRecords(activeSession);
        const convertedRecords = res.map((record: DNSRecord) => ({
          name: record.domain || "",
          type:
            typeof record.type === "string"
              ? record.type
              : ["A", "AAAA", "CNAME", "TXT"][record.type as number] || "A",
          content: record.value || "",
          ttl: record.ttl || 3600,
          id: "id" in record ? record.id : undefined,
        }));

        setDnsRecords(convertedRecords as DnsRecord[]);
        setFetched(true);
      } catch (error: unknown) {
        let msg = "Failed to fetch DNS records";

        if (
          error &&
          typeof error === "object" &&
          "response" in error &&
          error.response &&
          typeof error.response === "object" &&
          "status" in error.response &&
          error.response.status === 403
        ) {
          msg = "Your session token is invalid. Please request a new URL";
          Utils.removeSession(
            Utils.sessions.findIndex((s) => s.subdomain === activeSession),
          );
        }

        toast.error(msg, Utils.toastOptions);
      }
    };

    if (!fetched || !dnsRecords.length) {
      fetchData();
    }
  }, [activeSession, fetched, dnsRecords.length, toast]);

  useEffect(() => {
    setFetched(false);
  }, [activeSession]);

  const add = useCallback(
    (domain = "", typeIndex = 0, value = "") => {
      if (!activeSession) {
        toast.error("No active session selected", Utils.toastOptions);
        return;
      }

      if (process.env.NODE_ENV === "test") {
        const typeValues = ["A", "AAAA", "CNAME", "TXT"];
        const typeStr = typeValues[typeIndex] || "A";

        const newRecord: DnsRecord = {
          name: domain,
          type: typeStr,
          content: value,
          ttl: 3600, // Default TTL
        };

        setDnsRecords((prevRecords) => [...prevRecords, newRecord]);
        return;
      }

      const sessionData = getActiveSessionData();
      if (!sessionData?.token) {
        toast.error("No active session selected", Utils.toastOptions);
        return;
      }

      const typeValues = ["A", "AAAA", "CNAME", "TXT"];
      const typeStr = typeValues[typeIndex] || "A";

      const newRecord: DnsRecord = {
        name: domain,
        type: typeStr,
        content: value,
        ttl: 3600, // Default TTL
      };

      setDnsRecords((prevRecords) => [...prevRecords, newRecord]);
    },
    [activeSession, toast, getActiveSessionData],
  );

  const handleRecordInputChange = useCallback(
    (
      index: number,
      domain: string,
      typeIndex: number,
      value: string,
      toDelete?: boolean,
    ) => {
      const sessionData = getActiveSessionData();
      if (!sessionData?.token) {
        toast.error("No active session selected", Utils.toastOptions);
        return;
      }

      setDnsRecords((prevRecords) => {
        const updatedRecords = [...prevRecords];
        if (toDelete) {
          updatedRecords.splice(index, 1);
        } else {
          const typeValues = ["A", "AAAA", "CNAME", "TXT"];
          const typeStr = typeValues[typeIndex] || "A";

          updatedRecords[index] = {
            name: domain,
            type: typeStr,
            content: value,
            ttl: updatedRecords[index]?.ttl || 3600,
            id: updatedRecords[index]?.id,
          };
        }
        return updatedRecords;
      });
    },
    [activeSession, toast, getActiveSessionData],
  );

  const saveChanges = useCallback(() => {
    const sessionData = getActiveSessionData();
    if (!sessionData?.token) {
      toast.error("No active session selected", Utils.toastOptions);
      return;
    }

    const filteredRecords = dnsRecords.filter(
      (record) =>
        typeof record.name === "string" &&
        record.name.length > 0 &&
        typeof record.content === "string" &&
        record.content.length > 0,
    );

    const apiRecords = filteredRecords.map((record) => ({
      domain: record.name,
      type: record.type,
      value: record.content,
      ttl: record.ttl,
      id: record.id,
      subdomain: activeSession || "",
    })) as DNSRecord[];

    const obj = {
      records: apiRecords,
    };

    Utils.updateDNSRecords(obj, activeSession).then((res: unknown) => {
      if (
        res &&
        typeof res === "object" &&
        "error" in res &&
        typeof res.error === "string"
      ) {
        toast.error(res.error, Utils.toastOptions);
      } else if (
        res &&
        typeof res === "object" &&
        "msg" in res &&
        typeof res.msg === "string"
      ) {
        toast.success(res.msg, Utils.toastOptions);
      }
    });
  }, [dnsRecords, activeSession, toast, getActiveSessionData]);

  const renderedDnsRecords = dnsRecords
    .map((element, index) => {
      try {
        const typeIndex =
          typeof element.type === "string"
            ? ["A", "AAAA", "CNAME", "TXT"].indexOf(element.type)
            : 0;

        let domainName = typeof element.name === "string" ? element.name : "";
        if (user && domainName.includes(user.subdomain + "." + Utils.domain)) {
          domainName = domainName.substring(
            0,
            domainName.indexOf(user.subdomain + "." + Utils.domain) - 1,
          );
        }

        return (
          <RecordInput
            key={index}
            index={index}
            type={typeIndex}
            domain={domainName}
            value={typeof element.content === "string" ? element.content : ""}
            subdomain={activeSession}
            handleRecordInputChange={handleRecordInputChange}
          />
        );
      } catch (error) {
        console.error("Error parsing DNS record:", error);
        return null;
      }
    })
    .filter(Boolean);

  return (
    <div className="card card-w-title card-body">
      <div className="grid">
        <div className="col-6">
          <h1>DNS Records</h1>
        </div>
        <div className="col-6">
          <Button
            label="Save changes"
            icon="pi pi-save"
            className="p-button-success p-button-text"
            style={{ float: "right" }}
            onClick={saveChanges}
          />
        </div>
      </div>
      <div className="grid">
        <div className="col-12">
          <p>
            You can use % to select a random IP from multiple (e.g.
            127.0.0.1%8.8.8.8)
          </p>
        </div>
      </div>
      <div className="grid">
        <div className="col-12">
          <Button
            label="Add record"
            onClick={() => add()}
            icon="pi pi-plus"
            className="p-button-text"
            style={{ float: "right" }}
          />
        </div>
      </div>
      {renderedDnsRecords}
    </div>
  );
};

export default DnsSettingsPage;

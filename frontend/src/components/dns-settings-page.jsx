import React, { useState, useEffect, useCallback } from "react";
import { Button } from "primereact/button";
import { RecordInput } from "./record-input";
import { Utils } from "../utils";

export const DnsSettingsPage = ({
  dnsRecords: propDnsRecords = [],
  user,
  toast,
  activeSession,
}) => {
  const [dnsRecords, setDnsRecords] = useState(propDnsRecords);

  useEffect(() => {
    setDnsRecords(propDnsRecords);
  }, [propDnsRecords]);

  useEffect(() => {
    const fetchData = async () => {
      if (!activeSession?.subdomain) {
        setDnsRecords([]);
        return;
      }

      try {
        const token = Utils.getSessionToken(activeSession.subdomain);
        if (!token) {
          throw new Error("No valid token found for session");
        }

        const res = await Utils.getDNSRecords(activeSession.subdomain);
        setDnsRecords(res);
      } catch (error) {
        let msg;
        if (error.response?.status === 403) {
          msg = "Your session token is invalid. Please request a new URL";
          Utils.removeSession(activeSession.subdomain);
        } else {
          msg = "Failed to fetch DNS records";
        }
        toast.error(msg, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        });
      }
    };

    fetchData();
  }, [activeSession, toast]);

  const add = useCallback(
    (domain = "", type = 0, value = "") => {
      if (!activeSession?.token) {
        toast.error("No active session selected");
        return;
      }
      setDnsRecords((prevRecords) => [
        ...prevRecords,
        { domain, type, value, subdomain: activeSession.subdomain },
      ]);
    },
    [activeSession, toast],
  );

  const handleRecordInputChange = useCallback(
    (index, domain, type, value, toDelete) => {
      if (!activeSession?.token) {
        toast.error("No active session selected");
        return;
      }
      setDnsRecords((prevRecords) => {
        const updatedRecords = [...prevRecords];
        if (toDelete) {
          updatedRecords.splice(index, 1);
        } else {
          updatedRecords[index] = {
            domain,
            type,
            value,
            subdomain: activeSession.subdomain,
          };
        }
        return updatedRecords;
      });
    },
    [activeSession, toast],
  );

  const saveChanges = useCallback(() => {
    if (!activeSession?.token) {
      toast.error("No active session selected");
      return;
    }
    const filteredRecords = dnsRecords.filter(
      (value) => value.domain.length > 0 && value.value.length > 0,
    );
    const obj = {
      records: filteredRecords.map((record) => ({
        ...record,
        subdomain: activeSession.subdomain,
      })),
    };

    Utils.updateDNSRecords(obj, activeSession.subdomain).then((res) => {
      if (res.error) {
        toast.error(res.error, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        });
      } else {
        toast.success(res.msg, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        });
      }
    });
  }, [dnsRecords, toast, activeSession]);

  const renderedDnsRecords = dnsRecords.map((element, index) => {
    try {
      if (typeof element.type === "string") {
        element.type = ["A", "AAAA", "CNAME", "TXT"].indexOf(element.type);
      }
      if (
        element.domain.lastIndexOf(user.subdomain + "." + Utils.domain) >= 0
      ) {
        element.domain = element.domain.substr(
          0,
          element.domain.lastIndexOf(user.subdomain + "." + Utils.domain) - 1,
        );
      }
    } catch {
      console.error("Error parsing DNS record");
    }
    return (
      <RecordInput
        key={index}
        index={index}
        type={element.type}
        domain={element.domain}
        value={element.value}
        subdomain={
          element.subdomain || activeSession?.subdomain || user.subdomain
        }
        handleRecordInputChange={handleRecordInputChange}
      />
    );
  });

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

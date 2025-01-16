import React, { useState, useEffect, useCallback } from "react";
import { Button } from "primereact/button";
import { RecordInput } from "./record-input";
import { Utils } from "../utils";

export const DnsSettingsPage = ({
  dnsRecords: propDnsRecords = [],
  fetched = false,
  user,
  toast,
  activeSession,
}) => {
  const [dnsRecords, setDnsRecords] = useState(propDnsRecords);
  const [hasFetched, setHasFetched] = useState(fetched);

  useEffect(() => {
    const fetchData = async () => {
      try {
        const res = await Utils.getDNSRecords(activeSession);
        setDnsRecords(res);
        setHasFetched(true); // Update the state to indicate fetching is complete
      } catch (error) {
        let msg;
        if (error.response?.status === 403) {
          msg = "Your session token is invalid. Please request a new URL";
          Utils.removeSession(activeSession?.index);
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

    // Refetch when session changes or hasn't fetched yet
    if (!hasFetched || (activeSession && activeSession.token)) {
      setHasFetched(false);
      fetchData();
    }
  }, [hasFetched, toast, activeSession]);

  const add = (domain = "", type = 0, value = "") => {
    if (!activeSession?.token) {
      toast.error("No active session selected");
      return;
    }
    const newDnsRecords = [
      ...dnsRecords,
      { domain, type, value, subdomain: activeSession.subdomain },
    ];
    setDnsRecords(newDnsRecords);
  };

  const handleRecordInputChange = (index, domain, type, value, toDelete) => {
    if (!activeSession?.token) {
      toast.error("No active session selected");
      return;
    }
    const updatedDnsRecords = [...dnsRecords];
    if (toDelete) {
      updatedDnsRecords.splice(index, 1);
    } else {
      updatedDnsRecords[index] = { 
        domain, type, value, 
        subdomain: activeSession.subdomain 
      };
    }
    setDnsRecords(updatedDnsRecords);
  };

  const saveChanges = useCallback(() => {
    if (!activeSession?.token) {
      toast.error("No active session selected");
      return;
    }
    const filteredRecords = dnsRecords.filter(
      (value) => value.domain.length > 0 && value.value.length > 0,
    );
    const obj = { records: filteredRecords.map(record => ({ ...record, subdomain: activeSession.subdomain })) };

    Utils.updateDNSRecords(obj, activeSession).then((res) => {
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
      const currentSubdomain = activeSession?.subdomain || user.subdomain;
      if (
        element.domain.endsWith(currentSubdomain + "." + Utils.domain)
      ) {
        element.domain = element.domain.substr(
          0,
          element.domain.length - (currentSubdomain + "." + Utils.domain).length - 1
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
        subdomain={element.subdomain || activeSession?.subdomain || user.subdomain}
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

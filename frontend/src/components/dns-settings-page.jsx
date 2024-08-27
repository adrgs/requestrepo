import React, { useState, useEffect, useCallback } from "react";
import { Button } from "primereact/button";
import { RecordInput } from "./record-input";
import { Utils } from "../utils";

export const DnsSettingsPage = ({
  dnsRecords: propDnsRecords = [],
  fetched = false,
  user,
  toast,
}) => {
  const [dnsRecords, setDnsRecords] = useState(propDnsRecords);
  const [hasFetched, setHasFetched] = useState(fetched);

  useEffect(() => {
    const fetchData = async () => {
      try {
        const res = await Utils.getDNSRecords();
        setDnsRecords(res);
        setHasFetched(true); // Update the state to indicate fetching is complete
      } catch (error) {
        let msg = "Failed to fetch DNS records";
        if (error.response.status === 403) {
          msg = "Your session token is invalid. Please request a new URL";
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

    if (!hasFetched) {
      fetchData();
    }
  }, [hasFetched, toast]);

  const add = (domain = "", type = 0, value = "") => {
    const newDnsRecords = [
      ...dnsRecords,
      { domain, type, value, subdomain: user.subdomain },
    ];
    setDnsRecords(newDnsRecords);
  };

  const handleRecordInputChange = (index, domain, type, value, toDelete) => {
    const updatedDnsRecords = [...dnsRecords];
    if (toDelete) {
      updatedDnsRecords.splice(index, 1);
    } else {
      updatedDnsRecords[index] = { domain, type, value };
    }
    setDnsRecords(updatedDnsRecords);
  };

  const saveChanges = useCallback(() => {
    const filteredRecords = dnsRecords.filter(
      (value) => value.domain.length > 0 && value.value.length > 0,
    );
    const obj = { records: filteredRecords };

    Utils.updateDNSRecords(obj).then((res) => {
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
  }, [dnsRecords, toast]);

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
        subdomain={element.subdomain || user.subdomain}
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

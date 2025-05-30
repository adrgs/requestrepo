import React, { useState, useEffect } from "react";
import { Dropdown } from "primereact/dropdown";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import { Utils } from "../utils";

interface RecordInputProps {
  type: number;
  domain: string;
  value: string;
  index: number;
  subdomain: string | null;
  handleRecordInputChange: (
    index: number,
    domain: string,
    type: number,
    value: string,
    toDelete?: boolean,
  ) => void;
}

export const RecordInput: React.FC<RecordInputProps> = ({
  type = 0,
  domain = "",
  value = "",
  index = 0,
  subdomain,
  handleRecordInputChange,
}) => {
  const [recordType, setRecordType] = useState<number>(type);
  const [recordDomain, setRecordDomain] = useState<string>(domain);
  const [recordValue, setRecordValue] = useState<string>(value);

  useEffect(() => {
    setRecordType(type);
    setRecordDomain(domain);
    setRecordValue(value);
  }, [type, domain, value]);

  const changeEvent = (
    event:
      | React.ChangeEvent<HTMLInputElement>
      | { value: number }
      | React.MouseEvent<HTMLButtonElement>,
    action: string,
  ): void => {
    if (action === "domain") {
      if ("target" in event && "value" in event.target) {
        const newValue = event.target.value || "";
        handleRecordInputChange(
          index,
          newValue,
          recordType,
          recordValue,
          false,
        );
        setRecordDomain(newValue);
      }
    } else if (action === "type") {
      if ("value" in event) {
        handleRecordInputChange(
          index,
          recordDomain,
          event.value,
          recordValue,
          false,
        );
        setRecordType(event.value);
      }
    } else if (action === "value") {
      if ("target" in event && "value" in event.target) {
        const newValue = event.target.value || "";
        handleRecordInputChange(
          index,
          recordDomain,
          recordType,
          newValue,
          false,
        );
        setRecordValue(newValue);
      }
    } else if (action === "delete") {
      handleRecordInputChange(
        index,
        recordDomain,
        recordType,
        recordValue,
        true,
      );
    }
  };

  const DNSRecordTypes = [
    { label: "A", value: 0 },
    { label: "AAAA", value: 1 },
    { label: "CNAME", value: 2 },
    { label: "TXT", value: 3 },
  ];

  return (
    <div className="grid">
      <div className="col-3">
        <label>Record type: </label>
        <Dropdown
          value={recordType}
          options={DNSRecordTypes}
          onChange={(e) => {
            changeEvent(e, "type");
          }}
          placeholder="Select a record type"
        />
      </div>
      <div className="col">
        <label>URL: </label>
        <InputText
          value={recordDomain || ""}
          onChange={(e) => {
            if (e.target.value.length < 64) changeEvent(e, "domain");
          }}
        />
        .{typeof subdomain === "string" ? subdomain : ""}.{Utils.domain}
      </div>
      <div className="col">
        <label>Value: </label>
        <InputText
          style={{ maxWidth: "50%" }}
          value={recordValue || ""}
          onChange={(e) => {
            if (e.target.value.length < 256 && /^[ -~]+$/.test(e.target.value))
              changeEvent(e, "value");
          }}
        />
        <Button
          icon="pi pi-times"
          className="p-button-danger p-button-rounded p-button-text p-button-icon"
          onClick={(event) => changeEvent(event, "delete")}
        />
      </div>
    </div>
  );
};

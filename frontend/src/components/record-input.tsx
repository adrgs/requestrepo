import React, { Component } from "react";
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

interface RecordInputState {
  type: number;
  domain: string;
  value: string;
  index: number;
}

export class RecordInput extends Component<RecordInputProps, RecordInputState> {
  constructor(props: RecordInputProps) {
    super(props);
    this.state = {
      type: this.props.type ?? 0,
      domain: this.props.domain ?? "",
      value: this.props.value ?? "",
      index: this.props.index ?? 0,
    };
    this.changeEvent = this.changeEvent.bind(this);
  }

  changeEvent(
    event:
      | React.ChangeEvent<HTMLInputElement>
      | { value: number }
      | React.MouseEvent<HTMLButtonElement>,
    action: string,
  ): void {
    if (action === "domain") {
      if ("target" in event && "value" in event.target) {
        const value = event.target.value || "";
        this.props.handleRecordInputChange(
          this.props.index,
          value,
          this.state.type,
          this.state.value,
          false,
        );
        this.setState({ domain: value });
      }
    } else if (action === "type") {
      if ("value" in event) {
        this.props.handleRecordInputChange(
          this.props.index,
          this.state.domain,
          event.value,
          this.state.value,
          false,
        );
        this.setState({ type: event.value });
      }
    } else if (action === "value") {
      if ("target" in event && "value" in event.target) {
        const value = event.target.value || "";
        this.props.handleRecordInputChange(
          this.props.index,
          this.state.domain,
          this.state.type,
          value,
          false,
        );
        this.setState({ value: value });
      }
    } else if (action === "delete") {
      this.props.handleRecordInputChange(
        this.props.index,
        this.state.domain,
        this.state.type,
        this.state.value,
        true,
      );
    }
  }

  UNSAFE_componentWillReceiveProps(newProps: RecordInputProps): void {
    this.setState({
      domain: newProps.domain,
      type: newProps.type,
      value: newProps.value,
      index: newProps.index,
    });
  }

  shouldComponentUpdate(nextProps: RecordInputProps): boolean {
    return (
      this.state.domain !== nextProps.domain ||
      this.state.type !== nextProps.type ||
      this.state.value !== nextProps.value ||
      this.state.index !== nextProps.index
    );
  }

  render(): React.ReactNode {
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
            value={this.state.type}
            options={DNSRecordTypes}
            onChange={(e) => {
              this.changeEvent(e, "type");
            }}
            placeholder="Select a record type"
          />
        </div>
        <div className="col">
          <label>URL: </label>
          <InputText
            value={this.state.domain || ""}
            onChange={(e) => {
              if (e.target.value.length < 64) this.changeEvent(e, "domain");
            }}
          />
          .
          {typeof this.props.subdomain === "string" ? this.props.subdomain : ""}
          .{Utils.domain}
        </div>
        <div className="col">
          <label>Value: </label>
          <InputText
            style={{ maxWidth: "50%" }}
            value={this.state.value || ""}
            onChange={(e) => {
              if (
                e.target.value.length < 256 &&
                /^[ -~]+$/.test(e.target.value)
              )
                this.changeEvent(e, "value");
            }}
          />
          <Button
            icon="pi pi-times"
            className="p-button-danger p-button-rounded p-button-text p-button-icon"
            onClick={(event) => this.changeEvent(event, "delete")}
          />
        </div>
      </div>
    );
  }
}

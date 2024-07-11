import React, { Component } from "react";
import { Dropdown } from "primereact/dropdown";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import { Utils } from "../utils";

export class RecordInput extends Component {
  constructor(props) {
    super(props);
    this.state = {
      type: this.props.type ? this.props.type : 0,
      domain: this.props.domain ? this.props.domain : "",
      value: this.props.value ? this.props.value : "",
      index: this.props.index ? this.props.index : 0,
    };
    this.changeEvent = this.changeEvent.bind(this);
  }

  changeEvent(event, action) {
    if (action === "domain") {
      this.props.handleRecordInputChange(
        this.props.index,
        event.target.value || "",
        this.state.type,
        this.state.value,
        false,
      );
      this.setState({ domain: event.target.value || "" });
    } else if (action === "type") {
      this.props.handleRecordInputChange(
        this.props.index,
        this.state.domain,
        event.value,
        this.state.value,
        false,
      );
      this.setState({ type: event.value });
    } else if (action === "value") {
      this.props.handleRecordInputChange(
        this.props.index,
        this.state.domain,
        this.state.type,
        event.target.value || "",
        false,
      );
      this.setState({ value: event.target.value || "" });
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

  UNSAFE_componentWillReceiveProps(newProps) {
    this.setState({
      domain: newProps.domain,
      type: newProps.type,
      value: newProps.value,
      index: newProps.index,
    });
  }

  shouldComponentUpdate(nextProps) {
    return (
      this.state.domain !== nextProps.domain ||
      this.state.type !== nextProps.type ||
      this.state.value !== nextProps.value ||
      this.state.index !== nextProps.index
    );
  }

  render() {
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
          .{this.props.subdomain}.{Utils.domain}
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

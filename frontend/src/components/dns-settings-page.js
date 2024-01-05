import React, { Component } from "react";
import { Button } from "primereact/button";
import { RecordInput } from "./record-input";
import { Utils } from "../utils";

export class DnsSettingsPage extends Component {
  constructor(props) {
    super(props);

    this.state = {
      dnsRecords: this.props.dnsRecords ? this.props.dnsRecords : [],
    };

    if (this.props.fetched === false) {
      Utils.getDNSRecords().then((res) => {
        this.setState({ dnsRecords: res });
      });
    }

    this.add = this.add.bind(this);
    this.handleRecordInputChange = this.handleRecordInputChange.bind(this);
    this.saveChanges = this.saveChanges.bind(this);
  }

  add(domain, type, value) {
    if (typeof domain !== "string") domain = "";
    if (typeof value !== "string") value = "";
    if (typeof type !== "number") type = 0;
    const { dnsRecords } = this.state;
    dnsRecords.push({ domain: domain, type: type, value: value, subdomain: this.props.user.subdomain });
    this.setState({ dnsRecords: dnsRecords });
  }

  handleRecordInputChange(index, domain, type, value, toDelete) {
    const dnsRecords = this.state.dnsRecords;
    if (toDelete === false) {
      dnsRecords[index] = { domain: domain, type: type, value: value };
    } else {
      dnsRecords.splice(index, 1);
    }
    this.setState({ dnsRecords: dnsRecords });
    this.setState(this.state);
  }

  saveChanges() {
    let obj = {};
    obj["records"] = this.state.dnsRecords.filter(function (value) {
      return value.domain.length > 0 && value.value.length > 0;
    });
    Utils.updateDNSRecords(obj).then((res) => {
      if (res.error) {
        this.props.toast.error(res.error, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
        });
      } else {
        this.props.toast.success(res.msg, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
        });
        Utils.getDNSRecords().then((res) => {
          this.setState({ dnsRecords: res });
        });
      }
    });
  }

  render() {
    let dnsRecords = [];

    dnsRecords = this.state.dnsRecords.map((element) => {
      try {
        if (typeof element.type === "string") {
          element.type = ["A", "AAAA", "CNAME", "TXT"].indexOf(element.type);
        }
        if (element.domain.lastIndexOf(this.props.user.subdomain + "." + Utils.siteUrl) >= 0) {
          element.domain = element.domain.substr(0, element.domain.lastIndexOf(this.props.user.subdomain + "." + Utils.siteUrl) - 1);
        }
      } catch {}
      return element;
    });

    dnsRecords = this.state.dnsRecords.map((element, index) => {
      return (
        <RecordInput
          key={index}
          index={index}
          type={element["type"]}
          domain={element["domain"]}
          value={element["value"]}
          subdomain={element["subdomain"] ? element["subdomain"] : this.props.user.subdomain}
          handleRecordInputChange={this.handleRecordInputChange}
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
            <Button label="Save changes" icon="pi pi-save" className="p-button-success p-button-text" style={{ float: "right" }} onClick={this.saveChanges} />
          </div>
        </div>
        <div className="grid">
          <div className="col-12">
            <p>You can use / to cycle through the IPs or % to select a random IP (e.g. 127.0.0.1/8.8.8.8 or 127.0.0.1%8.8.8.8)</p>
          </div>
        </div>
        <div className="grid">
          <div className="col-12">
            <Button label="Add record" onClick={this.add} icon="pi pi-plus" className="p-button-text" style={{ float: "right" }} />
          </div>
        </div>
        {dnsRecords}
      </div>
    );
  }
}

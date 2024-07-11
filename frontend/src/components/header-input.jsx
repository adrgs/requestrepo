import React, { Component } from "react";
import { AutoComplete } from "primereact/autocomplete";
import { Button } from "primereact/button";

export class HeaderInput extends Component {
  constructor(props) {
    super(props);
    this.state = {
      header: this.props.header,
      value: this.props.value,
      filteredHeaders: null,
    };
    this.filterHeader = this.filterHeader.bind(this);
    this.changeEvent = this.changeEvent.bind(this);
  }

  filterHeader = (event) => {
    if (this.props.headersData) {
      let results = this.props.headersData
        .filter((header) => {
          return (
            header.name.toLowerCase().indexOf(event.query.toLowerCase()) >= 0
          );
        })
        .map(function (header) {
          return header.name;
        });
      this.setState({ filteredHeaders: results });
    }
  };

  UNSAFE_componentWillReceiveProps(newProps) {
    this.setState({
      header: newProps.header,
      value: newProps.value,
    });
  }

  //shouldComponentUpdate(nextProps) {
  //    return this.state.header != nextProps.header || this.state.value != nextProps.value || this.filteredHeaders != null;
  //}

  changeEvent(event, header) {
    if (header === true) {
      this.props.handleHeaderInputChange(
        this.props.index,
        event.value,
        this.state.value,
        false,
      );
      this.setState({ header: event.value, filteredHeaders: null });
    } else if (header === false) {
      this.props.handleHeaderInputChange(
        this.props.index,
        this.state.header,
        event.value,
        false,
      );
      this.setState({ value: event.value });
    } else if (header === "delete") {
      this.props.handleHeaderInputChange(
        this.props.index,
        this.state.header,
        this.state.value,
        true,
      );
    }
  }

  isDesktop() {
    return window.innerWidth > 1180;
  }

  render() {
    return (
      <div className="grid">
        <div className={this.isDesktop() ? "col-6" : "col-12"}>
          <div className="grid">
            <div className="col-5">
              <AutoComplete
                width={"100%"}
                delay={0}
                minLength={1}
                placeholder="Header-Name"
                size={30}
                suggestions={this.state.filteredHeaders}
                completeMethod={this.filterHeader}
                value={this.state.header}
                onChange={(event) => this.changeEvent(event, true)}
              />
            </div>
            <div className="col-5">
              <AutoComplete
                width={"100%"}
                minLength={1}
                placeholder="Value"
                value={this.state.value}
                onChange={(event) => this.changeEvent(event, false)}
              />
            </div>
            <div className="col-2">
              <Button
                icon="pi pi-times"
                className="p-button-danger p-button-rounded p-button-text p-button-icon"
                onClick={(event) => this.changeEvent(event, "delete")}
              />
            </div>
          </div>
        </div>
      </div>
    );
  }
}

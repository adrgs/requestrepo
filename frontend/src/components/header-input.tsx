import React, { Component } from "react";
import { AutoComplete } from "primereact/autocomplete";
import { Button } from "primereact/button";

interface HeaderInputProps {
  header: string;
  value: string;
  index: number;
  headersData?: Array<{ name: string; value: string }>;
  handleHeaderInputChange: (
    index: number,
    header: string,
    value: string,
    isDelete: boolean,
  ) => void;
}

interface HeaderInputState {
  header: string;
  value: string;
  filteredHeaders: string[] | null;
}

export class HeaderInput extends Component<HeaderInputProps, HeaderInputState> {
  constructor(props: HeaderInputProps) {
    super(props);
    this.state = {
      header: this.props.header,
      value: this.props.value,
      filteredHeaders: null,
    };
    this.filterHeader = this.filterHeader.bind(this);
    this.changeEvent = this.changeEvent.bind(this);
  }

  filterHeader = (event: { query: string }): void => {
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

  UNSAFE_componentWillReceiveProps(newProps: HeaderInputProps): void {
    this.setState({
      header: newProps.header,
      value: newProps.value,
    });
  }

  changeEvent(
    event: { value: string } | React.MouseEvent<HTMLButtonElement>,
    header: boolean | "delete"
  ): void {
    if (header === true) {
      if ('value' in event) {
        this.props.handleHeaderInputChange(
          this.props.index,
          event.value,
          this.state.value,
          false,
        );
        this.setState({ header: event.value, filteredHeaders: null });
      }
    } else if (header === false) {
      if ('value' in event) {
        this.props.handleHeaderInputChange(
          this.props.index,
          this.state.header,
          event.value,
          false,
        );
        this.setState({ value: event.value });
      }
    } else if (header === "delete") {
      this.props.handleHeaderInputChange(
        this.props.index,
        this.state.header,
        this.state.value,
        true,
      );
    }
  }

  isDesktop(): boolean {
    return window.innerWidth > 1180;
  }

  render(): React.ReactNode {
    return (
      <div className="grid">
        <div className={this.isDesktop() ? "col-6" : "col-12"}>
          <div className="grid">
            <div className="col-5">
              <AutoComplete
                style={{ width: "100%" }}
                delay={0}
                minLength={1}
                placeholder="Header-Name"
                size={30}
                suggestions={this.state.filteredHeaders || []}
                completeMethod={this.filterHeader}
                value={this.state.header}
                onChange={(event) => this.changeEvent(event, true)}
              />
            </div>
            <div className="col-5">
              <AutoComplete
                style={{ width: "100%" }}
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

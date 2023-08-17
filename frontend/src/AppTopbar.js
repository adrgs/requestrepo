import React, { Component } from "react";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import GitHubButton from "react-github-btn";

export class AppTopbar extends Component {
  constructor(props) {
    super(props);
    this.state = { ...props };
    this.state.searchValue = "";
    this.handleSearchValueChange = this.handleSearchValueChange.bind(this);
    this.toggleTheme = this.toggleTheme.bind(this);
  }

  toggleTheme() {
    // add dark to body
    if (document.body.classList.contains("dark")) {
      document.body.classList.remove("dark");
      localStorage.setItem("theme", "light");
    } else {
      document.body.classList.add("dark");
      localStorage.setItem("theme", "dark");
    }
  }

  static defaultProps = {};

  static propTypes = {};

  handleSearchValueChange(event) {
    this.setState({ searchValue: event.target.value });
    this.state.updateSearchValue(event.target.value);
  }

  render() {
    return (
      <div className="layout-topbar clearfix">
        <div style={{ float: "left" }}>
          <a href="/#">
            <object data="/logo.svg" type="image/svg+xml" style={{ height: "30px" }}>requestrepo</object>
          </a>
        </div>
        <div style={{ float: "left", marginLeft: "1rem" }} className="desktop-only">
          <GitHubButton href="https://github.com/adrgs/requestrepo" data-color-scheme={localStorage.getItem("theme")} data-icon="octicon-star" data-size="large" data-show-count="true" aria-label="Star adrgs/requestrepo on GitHub">
            Star
          </GitHubButton>
        </div>
        <div className="layout-topbar-icons" style={{ maxWidth: "23%" }}>
          <span style={{ marginTop: "8px", width: "100%" }} className="layout-topbar-search">
            <InputText style={{ width: "100%" }} type="text" placeholder="Search" value={this.state.searchValue} onChange={this.handleSearchValueChange} />
            <span className="layout-topbar-search-icon pi pi-search" />
          </span>
        </div>
        <div style={{ float: "right", marginLeft: "1rem" }}>
        <Button
            icon={"pi pi-" + (document.body.classList.contains("dark") ?  "sun" : "moon")}
            className="p-button-text p-button-secondary"
            style={{"padding": "5px 10px 5px 10px", "marginRight": "20px"}}
            onClick={this.toggleTheme}
          />
        </div>
      </div>
    );
  }
}

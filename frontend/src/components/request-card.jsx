import React, { Component } from "react";

export class RequestCard extends Component {
  constructor(props) {
    super(props);
    this.state = { ...props };

    this.click = this.click.bind(this);
    this.delete = this.delete.bind(this);
  }

  shouldComponentUpdate() {
    return true;
  }

  click() {
    this.props.clickRequestAction("select", this.props.id);
  }

  delete() {
    this.props.clickRequestAction("delete", this.props.id);
  }

  render() {
    let annotation = null;
    if (this.props.new === true)
      annotation = <span className="count delete">NEW!</span>;
    return (
      <div
        className={
          "card request summary " +
          (this.props.active ? "active " : "") +
          (this.props.visited ? "visited " : "")
        }
        onClick={this.click}
      >
        {annotation}
        <span className={"count " + this.props.method.toLowerCase()}>
          {this.props.method}
        </span>
        <span className="title">{this.props.title}</span>
        <span className="count bigx" onClick={this.delete}>
          X
        </span>
        <span className="detail">
          {this.props.country && (
            <span
              style={{ marginRight: "5px" }}
              className={"fi fi-" + this.props.country.toLowerCase()}
            ></span>
          )}
          {this.props.detail}
          <span style={{ float: "right" }}>{this.props.time}</span>
        </span>
      </div>
    );
  }
}

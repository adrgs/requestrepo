import React, {Component} from 'react';
import {InputText} from 'primereact/inputtext';
import PropTypes from 'prop-types';
import GitHubButton from 'react-github-btn'

export class AppTopbar extends Component {

    constructor(props)
    {
        super(props);
        this.state = {...props};
        this.state.searchValue = "";
        this.handleSearchValueChange = this.handleSearchValueChange.bind(this);
    }

    static defaultProps = {
    }

    static propTypes = {
    }

    handleSearchValueChange(event) {
        this.setState({searchValue:event.target.value});
        this.state.updateSearchValue(event.target.value);
    }

    render() {
        return (
            <div className="layout-topbar clearfix">
                <div style={{"float":"left"}}>
                <a href={"#"}>
                    <object data="/logo.svg" type="image/svg+xml" style={{height:"30px"}}></object>
                </a>
                </div>
                <div style={{"float":"left", marginLeft:"1rem"}} className="desktop-only">
                    <GitHubButton href="https://github.com/adrgs/requestrepo" data-icon="octicon-star" data-size="large" data-show-count="true" aria-label="Star adrgs/requestrepo on GitHub">Star</GitHubButton>
                </div>
                <div className="layout-topbar-icons" style={{maxWidth:"40%"}}>
                    <span style={{marginTop:"8px", "width":"100%"}} className="layout-topbar-search">
                        <InputText style={{"width":"100%"}} type="text" placeholder="Search" value={this.state.searchValue} onChange={this.handleSearchValueChange}/>
                        <span className="layout-topbar-search-icon pi pi-search"/>
                    </span>
                </div>
            </div>
        );
    }
}
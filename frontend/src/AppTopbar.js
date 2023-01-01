import React, {Component} from 'react';
import {InputText} from 'primereact/inputtext';
import PropTypes from 'prop-types';

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
                <a href={"#"}>
                <div style={{"float":"left"}}>
                    <h2 style={{"color":"black"}}><span style={{"color":"#007ad9"}}>request</span>repo</h2>
                </div>
                </a>
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
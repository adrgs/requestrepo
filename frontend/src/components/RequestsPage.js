import React, {Component} from 'react';
import {Button} from 'primereact/button';
import {InputText} from 'primereact/inputtext';
import {Accordion, AccordionTab} from "primereact/accordion";
import {TabPanel, TabView} from "primereact/tabview";
import {Toolbar} from "primereact/toolbar";
import {RequestInfo} from "./RequestInfo";

export class RequestsPage extends Component {
    constructor(props)
    {
        super(props);
        this.state = {...props};
    }


    render() {
        return (
                <div className="card card-w-title" style={{"border":"1px solid #cccccc",borderTop:"0px",borderRadius:"0px 0px 5px 5px"}}>
                    {(this.props.user!==null && this.props.user!==undefined) && this.props.user.requests[this.props.user.selectedRequest] === undefined &&
                        <div className="p-grid">
                            <div className="p-col-12">
                                <h1>Awaiting requests</h1>
                                <p>How to make a request:</p>
                                <code>curl http://{this.props.user.url}</code>
                                <br /><br/>
                                <code>curl http://{this.props.user.domain}/{this.props.user.subdomain}/</code>
                                <br /><br/>
                                <code>curl -X POST --data hello http://{this.props.user.url}</code>
                                <br /><br/>
                                <code>nslookup your.data.here.{this.props.user.url}</code>
                                <br /><br/>
                                <code>echo RCE | curl -d @- {this.props.user.url}</code>
                                <br /><br/>
                                <p>Check out the Response tab to edit your HTTP Response or the DNS tab to add DNS records for this subdomain.</p>
                            </div>
                        </div>
                    }
                    {(this.props.user!==null && this.props.user!==undefined) && this.props.user.requests[this.props.user.selectedRequest] !== undefined &&
                        <RequestInfo request={this.props.user.requests[this.props.user.selectedRequest]} />
                    }
                </div>
        );
    }
}
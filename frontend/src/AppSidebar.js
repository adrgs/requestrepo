import React, { Component, useState } from 'react';
import {RequestCard} from "./components/RequestCard";
import {Checkbox} from 'primereact/checkbox';
import {Button} from 'primereact/button';
import {Utils} from "./Utils";

export class AppSidebar extends Component {

    constructor(props) {
        super(props);
        this.state = {
            http_filter: true,
            dns_filter: true,
        };

        this.lastNumberOfReqs = 0;
        this.numberOfReqs = 0;

        console.log(this.props);
        this.onCheckboxChange = this.onCheckboxChange.bind(this);
        this.hasValue = this.hasValue.bind(this);
        this.deleteAllRequests = this.deleteAllRequests.bind(this);
    }

    componentWillReceiveProps({someProp}) {
        this.forceUpdate();
        this.setState({...this.state,someProp})
    }

    scrollToBottom() {
        this.messagesEnd.scrollIntoView({ behavior: "auto" });
    }

    componentDidMount() {
        this.scrollToBottom();
    }

    componentDidUpdate() {
        if (this.numberOfReqs > this.lastNumberOfReqs)
            this.scrollToBottom();
        this.lastNumberOfReqs = this.numberOfReqs;
    }

    shouldComponentUpdate(nextProps, nextState, nextContext) {
        return true;
    }

    onCheckboxChange(event){
        if (event.value == 'HTTP') {
            this.state.http_filter = !this.state.http_filter;
        } else if (event.value == 'DNS') {
            this.state.dns_filter = !this.state.dns_filter;
        }
    }

    convertUTCDateToLocalDate(date) {
        //var newDate = new Date(date.getTime()+date.getTimezoneOffset()*60*1000);
        //var offset = date.getTimezoneOffset() / 60;
        //var hours = date.getHours();
        //newDate.setHours(hours - offset);
        //return newDate;
        return date;
    }

    getRequests() {
        let requests = [];
        let user = this.props.user;

        if (user.httpRequests !== null && user.dnsRequests !== null) {
            let i=0, j=0;
            while (i < user.httpRequests.length || j < user.dnsRequests.length) {
                let obj = {
                    title: null,
                    method: null,
                    time: null,
                    detail: null,
                    id: null,
                    type:null
                };

                let dateA = 0;
                let dateB = 0;
                if (i<user.httpRequests.length) {
                    dateA = parseInt(user.httpRequests[i].date);
                }
                if (j<user.dnsRequests.length)
                {
                    dateB = parseInt(user.dnsRequests[j].date);
                }

                if ((j >= user.dnsRequests.length || dateA < dateB) && (i < user.httpRequests.length)) {
                    let req = user.requests[user.httpRequests[i]['_id']];
                    obj['title'] = req['path'];
                    obj['method'] = req['method'];
                    obj['time'] = this.convertUTCDateToLocalDate(new Date(dateA*1000));
                    obj['time'] = obj['time'].toLocaleString();
                    obj['detail'] = req['ip'];
                    obj['id'] = req['_id'];
                    obj['key'] = obj['id'];
                    obj['type'] = 'HTTP';
                    obj['new'] = req['new'];

                    requests.push(obj);
                    i++;
                } else {
                    let req = user.requests[user.dnsRequests[j]['_id']];
                    obj['title'] = req['name'];
                    obj['method'] = 'DNS';
                    obj['time'] = this.convertUTCDateToLocalDate(new Date(dateB*1000));
                    obj['time'] = obj['time'].toLocaleString();
                    obj['detail'] = req['ip'];
                    obj['id'] = req['_id'];
                    obj['key'] = obj['id'];
                    obj['type'] = 'DNS';
                    obj['new'] = req['new'];

                    requests.push(obj);

                    j++;
                }
            }
        }
        this.numberOfReqs = requests.length;

        return requests;
    }

    deleteAllRequests()
    {
        let requests = this.getRequests();
        for (var i=0;i<requests.length;i++)
        {
            this.props.clickRequestAction('delete', requests[i].id);
        }
    }

    hasValue(item, needle) {
        if (needle === "" || needle === undefined || needle === null) return true;

        if (item['name'] !== undefined) {
            if ('dns'.indexOf(needle) >= 0) return true;
        } else {
            if ('http'.indexOf(needle) >= 0) return true;
        }
        needle = needle.toLowerCase();
        for (let property in item)
        {
            let val = item[property];
            if (property == 'raw') {
                val = atob(item[property]).toString().toLowerCase();
                if (val.indexOf(needle) >= 0) return true;
                continue;
            }
            if (property == 'date') {
                val = this.convertUTCDateToLocalDate(new Date(parseInt(val)*1000)).toLocaleString().toLowerCase();
                console.log(val);
                if (val.indexOf(needle) >= 0) return true;
                continue;
            }

            if (typeof val === 'object') {
                for (let prop in val) {
                    let val2 = val[prop].toString().toLowerCase();
                    let val3 = prop.toString().toLowerCase();
                    if (val2.indexOf(needle) >= 0) return true;
                    if (val3.indexOf(needle) >= 0) return true;
                }
            } else {
                val = val.toString().toLowerCase();
                if (val.indexOf(needle) >= 0) return true;
            }
        }

        return false;
    }

    isDesktop() {
        return window.innerWidth > 1024;
    }

    render() {
        let requests = this.getRequests();
        let hasValue = this.hasValue;
        let user = this.props.user;
        let searchValue = this.props.searchValue;
        let dns_filter = this.state.dns_filter;
        let http_filter = this.state.http_filter;
        requests = requests.filter(function (item, index, arr) {
            return hasValue(user.requests[item.id], searchValue) && ((item.type==='DNS' && dns_filter) || (item.type==='HTTP' && http_filter));
        });
        let good = false;
        for (let i=0;i<requests.length;i++)
        {
            if (this.props.user.selectedRequest == requests[i].id) {
                good = true;
                break;
            }
        }
        if (!good) {
            if (requests.length>0) {
                this.props.clickRequestAction('select', requests[0].id);
            } else {
                this.props.clickRequestAction('reset', undefined);
            }
        }
        return (
            <div className={"layout-sidebar layout-sidebar-light"} >
                <div className={"layout-sidebar-header"}>
                    <div style={{"textAlign":"center","padding":"5px","borderBottom":"1px solid #ccc"}}>
                        <Button label="Delete all requests" icon="pi pi-times" className="p-button-danger p-button-text" onClick={this.deleteAllRequests} />
                    </div>
                    <div style={{"padding":"0.85rem"}}>
                    <b style={{marginRight:"20px"}}>Requests ({requests.length})</b>
                    <Checkbox value="HTTP" inputId="cbHTTP" onChange={this.onCheckboxChange} checked={this.state.http_filter} />
                    <label style={{marginRight:"15px"}} htmlFor="cbHTTP" className="p-checkbox-label">HTTP</label>
                    <Checkbox value="DNS" inputId="cbDNS" onChange={this.onCheckboxChange} checked={this.state.dns_filter} />
                    <label htmlFor="cbDNS" className="p-checkbox-label">DNS</label>
                    </div>
                </div>
                <div className="requests-box">
                    { requests.map( (item, index) => {
                        return <RequestCard active={this.props.user.selectedRequest === item.id} visited={this.props.user.visited[item.id]!==undefined}
                                            title={item.title} time={item.time} new={item.new}
                                            method={item.method} detail={item.detail} id={item.id}
                                            key={item.key} clickRequestAction={this.props.clickRequestAction}/>
                    })}
                    <div style={{ float:"left", clear: "both" }}
                         ref={(el) => { this.messagesEnd = el; }}>
                    </div>
                </div>
                {this.isDesktop() &&
                    <div style={{position:"absolute", bottom:"0", height:"100px", textAlign:"center", width:"100%"}}>
                    <Button style={{margin:"0 auto", display:"block"}} label="See on GitHub" icon="pi pi-github" className="p-button-plain p-button-text" onClick={() => {window.open("https://github.com/adrgs/requestrepo")}} />
                    </div>
                }
            </div>
        );
    }
}
import React, {Component, ReactDOM} from 'react';
import HexEditor from 'react-hex-editor';
import {InputText} from 'primereact/inputtext';

export class RequestInfo extends Component {
    constructor(props)
    {
        super(props);
        this.state = {...props};
    }

    updateDimensions = () => {
        this.setState(this.state);
    };
    componentDidMount() {
        window.addEventListener('resize', this.updateDimensions);
    }
    componentWillUnmount() {
        window.removeEventListener('resize', this.updateDimensions);
    }

    convertUTCDateToLocalDate(date) {
        //var newDate = new Date(date.getTime()+date.getTimezoneOffset()*60*1000);
        //var offset = date.getTimezoneOffset() / 60;
        //var hours = date.getHours();
        //newDate.setHours(hours - offset);
        //return newDate;
        return date;
    }

    isDesktop() {
        return window.innerWidth > 1024;
    }

    render() {
        let request = this.props.request;
        let data = atob(request.raw);

        let headerKeys;
        if (request.headers)
            headerKeys = Object.keys(request.headers);

        if (request.name===undefined) {
            data = request.method+ " " + request.path + " " + request.protocol + "\r\n";
            data += 'Host' + ": " + request.headers['Host'] + "\r\n";
            headerKeys.map( (item, index) => {
                if (item !== 'Host') {
                    data += item + ": " + request.headers[item] + "\r\n";
                }
            });
            data += "\r\n";
            if (request.raw != "")
                data += atob(request.raw) + "\r\n";
        }

        let out;

        if (request.name===undefined)
        {
            out =  <div className="grid">
                <div className="col-12">
                    <h1>Request Details</h1>
                    <table className="req-table">
                        <tbody>
                        <tr>
                            <td className="req-table-a">Request Type</td>
                            <td>
                                <span style={{"position":"static"}} className="count other">{request.protocol}</span>
                                <span style={{"position":"static"}} className={"count " + request.method.toLowerCase()}>{request.method}</span>
                            </td>
                        </tr>
                        <tr>
                            <td className="req-table-a">URL</td>
                            <td className="req-table-b"><a href={request.url}>{request.url}</a></td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Sender IP</td>
                            <td className="req-table-b">{request.ip}</td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Date</td>
                            <td className="req-table-b">{this.convertUTCDateToLocalDate(new Date(parseInt(request.date)*1000)).toLocaleString()}</td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Path</td>
                            <td className="req-table-b">{request.path}</td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Query string</td>
                            <td className="req-table-b">{request.query}</td>
                        </tr>
                        </tbody>
                    </table>
                </div>
                <div className="col-12">
                    <h1>Headers</h1>
                    <table className="req-table">
                        <tbody>
                        {
                            headerKeys.map( (item, index) => {
                                return (<tr key={index}>
                                    <td className="req-table-a">{item}</td>
                                    <td className="req-table-b">{request.headers[item]}</td>
                                </tr>);
                            })
                        }
                        </tbody>
                    </table>
                </div>
                <div className="col-12">
                    <h1>Query Parameters</h1>
                    {request.query
                        ?   <table className="req-table">
                            {
                                request.query.substring(1).split('&').map( (dict, index) => {
                                    let q = dict.split('=');
                                    return (<tr key={index}>
                                        <td className="req-table-a">{q[0]}</td>
                                        <td className="req-table-b">{q[1]}</td>
                                    </tr>);
                                })
                            }
                            </table>
                        : <p>(empty)</p>
                    }
                </div>
                <div className="col-12">
                    <h1>Form Data</h1>
                    {request.raw
                        ? <div>
                            <InputText type="text" style={{"width":"100%"}} value={request.raw}/>
                            <br />
                            <pre style={{"maxHeight":"400px"}}>{atob(request.raw)}</pre>
                         </div>
                        : <p>(empty)</p>
                    }
                </div>
                <div className="col-12 raw-req">
                    <h1>Raw request</h1>
                    <InputText type="text" style={{"width":"100%"}} value={btoa(data)}/>
                    <br />
                    <pre style={{"overflowWrap":"break-word","padding":"10px"}}>{data}</pre>
                </div>
            </div>
        } else {
            out =  <div className="grid">
                <div className="col-12">
                    <h1>Request Details</h1>
                    <table className="req-table">
                        <tbody>
                        <tr>
                            <td className="req-table-a">Request Type</td>
                            <td><span style={{"position":"static"}} className="count dns">DNS</span></td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Hostname</td>
                            <td className="req-table-b">{request.name}</td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Sender IP</td>
                            <td className="req-table-b">{request.ip}</td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Date</td>
                            <td className="req-table-b">{this.convertUTCDateToLocalDate(new Date(parseInt(request.date)*1000)).toLocaleString()}</td>
                        </tr>
                        <tr>
                            <td className="req-table-a">Type</td>
                            <td className="req-table-b">{request.type}</td>
                        </tr>
                        </tbody>
                    </table>
                </div>
                <div className="col-12">
                    <h1>Reply</h1>
                    <pre style={{"overflowWrap":"break-word"}}>{request.reply}</pre>
                </div>
                <div className="col-12 raw-req">
                    <h1>Raw request</h1>
                    <InputText type="text" style={{"width":"100%"}} value={request.raw}/>
                    <br />
                    <pre style={{"overflowWrap":"break-word","padding":"10px"}}>{data}</pre>
                </div>
            </div>
        }

        return (
            <div>{ out }</div>
        );
    }
}
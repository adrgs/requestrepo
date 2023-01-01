import React, {Component, useEffect} from 'react';
import classNames from 'classnames';
import {AppTopbar} from './AppTopbar';
import {AppSidebar} from './AppSidebar';
import {Route, Routes, useLocation, HashRouter} from 'react-router-dom';
import {RequestsPage} from './components/RequestsPage';
import {EditResponsePage} from './components/EditResponsePage';
import {DnsSettingsPage} from './components/DnsSettingsPage';
import 'primereact/resources/themes/lara-light-blue/theme.css';
import 'primereact/resources/primereact.min.css';
import 'primeicons/primeicons.css';
import 'primeflex/primeflex.css';
import './App.scss';
import {Toolbar} from "primereact/toolbar";
import {Button} from "primereact/button";
import {InputText} from "primereact/inputtext";
import {Utils} from "./Utils";
import { ToastContainer, toast } from 'react-toastify';
import 'react-toastify/dist/ReactToastify.css';
import axios from "axios";

class App extends Component {

    constructor() {
        super();

        if (!Utils.userHasSubdomain()) {
            Utils.getRandomSubdomain();
        }

        this.user = {
            url: Utils.getUserURL(),
            subdomain: Utils.subdomain,
            httpRequests: [],
            dnsRequests: [],
            timestamp: null,
            requests: {},
            visited: {},
            selectedRequest: localStorage.getItem('lastSelectedRequest')
        };

        this.state = {
            layoutMode: 'static',
            layoutColorMode: 'light',
            staticMenuInactive: false,
            overlayMenuActive: false,
            mobileMenuActive: false,
            user: this.user,
            searchValue: "",
            response: {"raw":"", headers:[], status_code:200, fetched:false},
            dnsRecords: [],
            dnsFetched: false
        };

        this.user.visited = JSON.parse(localStorage.getItem("visited") === null ? '{"length":0}' : localStorage.getItem("visited"));

        Utils.getDNSRecords().then(res => {
           this.setState({dnsRecords: res});
           this.setState({dnsFetched: true});
        });

        Utils.getFile().then(res => {
            let response = this.state.response;
            try {
                let decoded = atob(res.raw);
                res.raw = decoded;
            } catch { }

            response = res;
            res.fetched=true;
            this.setState({response:res});
            this.setState(this.state);
        });

        Utils.getDNSRecords().then(res => {
           this.setState({dnsRecords: res});
        });

        Utils.getRequests().then(res=> {
            this.user['httpRequests'] = this.user['httpRequests'].concat(res['http']);
            for (let i=0;i<this.user['httpRequests'].length;i++)
            {
                this.user.requests[this.user['httpRequests'][i]['_id']] = this.user['httpRequests'][i];
                this.user['httpRequests'][i]['new'] = false;
            }

            this.user['dnsRequests'] = this.user['dnsRequests'].concat(res['dns']);

            for (let i=0;i<this.user['dnsRequests'].length;i++)
            {
                this.user.requests[this.user['dnsRequests'][i]['_id']] = this.user['dnsRequests'][i];
                this.user['dnsRequests'][i]['new'] = false;
            }

            this.user['timestamp'] = res['date'];
            this.updateTitle();
            this.setState({ state: this.state });
        });

        this.onWrapperClick = this.onWrapperClick.bind(this);
        this.onToggleMenu = this.onToggleMenu.bind(this);
        this.onSidebarClick = this.onSidebarClick.bind(this);
        this.onMenuItemClick = this.onMenuItemClick.bind(this);
        this.updateFunction = this.updateFunction.bind(this);
        this.clickRequestAction = this.clickRequestAction.bind(this);
        this.copyUrl = this.copyUrl.bind(this);
        this.newUrl = this.newUrl.bind(this);
        this.updateTitle = this.updateTitle.bind(this);
        this.updateSearchValue = this.updateSearchValue.bind(this);
        this.createMenu();
    }

    clickRequestAction(action, id)
    {
        if (action == 'select') {
            if (this.state.user.requests[id] !== undefined) {
                this.state.user.selectedRequest = id;
                this.state.user.requests[id]['new'] = false;
                if (this.state.user.visited[id] === undefined) {
                    this.state.user.visited[id] = true;
                    this.state.user.visited['length'] = (this.state.user.visited['length'] !== undefined ? this.state.user.visited['length'] + 1 : 1);
                    localStorage.setItem("visited", JSON.stringify(this.state.user.visited));
                }
                localStorage.setItem('lastSelectedRequest', id);
            }
        }
        else if (action == 'delete') {
            let type = (this.state.user.requests[id].name===undefined ? "HTTP" : "DNS");
            this.state.user.requests[id] = undefined;
            this.state.user.visited[id] = undefined;
            if (this.state.user.visited.length!==undefined) {
                this.state.user.visited.length--;
            } else {
                this.state.user.visited.length = 0;
            }
            if (this.state.user.visited.length < 0 )
            {
                this.state.user.visited.length = 0;
            }
            localStorage.setItem("visited", JSON.stringify(this.state.user.visited));

            this.state.user.httpRequests = this.state.user.httpRequests.filter(function (value, index, arr) {
                return (value['_id'] !== id);
            });
            this.state.user.dnsRequests = this.state.user.dnsRequests.filter(function (value, index, arr) {
                return (value['_id'] !== id);
            });

            Utils.deleteRequest(id, type);

            this.setState({ state: this.state });
        } else if (action == 'reset') {
            this.state.user.selectedRequest = undefined;
            localStorage.setItem('lastSelectedRequest', undefined);
        }
        this.updateTitle();
    }

    isDesktop() {
        return window.innerWidth > 1024;
    }

    updateTitle() {
        let n = (this.state.user.httpRequests.length + this.state.user.dnsRequests.length) - (this.state.user.visited.length !== undefined ? this.state.user.visited.length : 0);
        let text = "Dashboard - requestrepo.com";
        if (n<=0) {
            document.title = text;
        } else {
            document.title = "(" + n + ") " + text;
        }
    }

    updateSearchValue(val) {
        this.state.searchValue = val;
        this.setState({ state: this.state });
    }

    updateFunction() {
        let user = this.state.user;
        if (!user) return;

        let newRequests = 0;

        let shouldUpdate = false;

        Utils.getRequests(user['timestamp']).then(res => {
            let ts = parseInt(user['timestamp']);

            var arr = ['http', 'dns'];

            for (var idx=0;idx<arr.length;idx++) {
                for (let i = 0; i < res[arr[idx]].length; i++) {
                    if (this.user.requests[res[arr[idx]][i]['_id']] !== undefined) continue;

                    res[arr[idx]][i]['new'] = true;
                    this.user.requests[res[arr[idx]][i]['_id']] = res[arr[idx]][i];
                    newRequests += 1;
                }
            }
            user['httpRequests'] = user['httpRequests'].concat(res['http']);
            user['dnsRequests'] = user['dnsRequests'].concat(res['dns']);

            let t2 = parseInt(res.date);
            if (t2>=ts) {
                user['timestamp'] = (t2+1).toString();
            }

            if (newRequests > 0)
            {
                this.setState({user});
                toast('ðŸš€ ' + newRequests + ' new requests!', {
                    position: "bottom-center",
                    autoClose: 4000,
                    hideProgressBar: false,
                    closeOnClick: true,
                    pauseOnHover: true,
                    draggable: true
                });
            }
            this.updateTitle();
        });
    }

    copyUrl(e)
    {
        this.urlArea.select();
        document.execCommand('copy');
        toast.info('URL copied to clipboard!', {
            position: "bottom-center",
            autoClose: 2500,
            hideProgressBar: false,
            closeOnClick: true,
            pauseOnHover: true,
            draggable: true
        });
    }

    newUrl(e)
    {
        Utils.getRandomSubdomain();
    }

    componentDidMount() {
        this.interval = setInterval(this.updateFunction, 5000);
    }

    componentWillUnmount() {
        clearInterval(this.interval);
    }

    componentDidUpdate() {
        if (this.state.mobileMenuActive)
            this.addClass(document.body, 'body-overflow-hidden');
        else
            this.removeClass(document.body, 'body-overflow-hidden');
    }

    render() {
        const logo = this.state.layoutColorMode === 'dark' ? 'assets/layout/images/logo-white.svg': 'assets/layout/images/logo.svg';

        const wrapperClass = classNames('layout-wrapper', {
            'layout-overlay': this.state.layoutMode === 'overlay',
            'layout-static': this.state.layoutMode === 'static',
            'layout-static-sidebar-inactive': this.state.staticMenuInactive && this.state.layoutMode === 'static',
            'layout-overlay-sidebar-active': this.state.overlayMenuActive && this.state.layoutMode === 'overlay',
            'layout-mobile-sidebar-active': this.state.mobileMenuActive
        });

        return (
            <div className={wrapperClass} onClick={this.onWrapperClick}>
                <AppTopbar onToggleMenu={this.onToggleMenu} updateSearchValue={this.updateSearchValue}/>

                <AppSidebar user={this.state.user} searchValue={this.state.searchValue} clickRequestAction={this.clickRequestAction}/>

                <div className="layout-main">
                    <div className="p-grid">
                        <div className="p-col-12">
                            <Toolbar style={{lineHeight:"3",borderColor:"#cccccc",borderRadius:"5px 5px 0px 0px"}} left={
                                <div style={{textAlign:"center"}}>
                                    <a href="#/requests"><Button  label="Requests" icon="pi pi-arrow-down" className="p-button-text p-button-secondary" style={{marginRight:'.25em'}} /></a>
                                    <a href="#/edit-response"><Button href="#/edit-response" label="Response" icon="pi pi-pencil" className="p-button-text p-button-secondary" style={{marginRight:'.25em'}} /></a>
                                    <a href="#/dns-settings"><Button href="#/dns-settings" label="DNS" icon="pi pi-home" className="p-button-text p-button-secondary" /></a>
                                </div>
                            } right={
                                <div style={{textAlign:"center"}}>
                                    <InputText type="text" placeholder="Your URL" value={this.state.user.url} style={{"width":"300px", marginRight:"1em"}} ref={(urlArea) => this.urlArea = urlArea} />
                                    <Button label="Copy URL" icon="pi pi-copy" className="p-button-success" style={{marginRight:'.25em'}} onClick={this.copyUrl} />
                                    <Button label="New URL" icon="pi pi-refresh" onClick={this.newUrl} />
                                </div>
                            }/>
                            <Routes>
                                <Route exact path="/" element={<RequestsPage user={this.state.user} />} />
                                <Route path="/requests" element={<RequestsPage user={this.state.user} />} />
                                <Route path="/edit-response" element={<EditResponsePage content={this.state.response.raw} statusCode={this.state.response['status_code']} headers={this.state.response.headers} user={this.state.user} fetched={this.state.response.fetched} toast={toast} />} />
                                <Route path="/dns-settings" element={<DnsSettingsPage  user={this.state.user} dnsRecords={this.state.dnsRecords} toast={toast} fetched={this.state.dnsFetched} />} />
                            </Routes>
                        </div>
                    </div>
                </div>

                <ToastContainer />
                <div className="layout-mask"></div>
            </div>
        );
    }

    onWrapperClick(event) {
        if (!this.menuClick) {
            this.setState({
                overlayMenuActive: false,
                mobileMenuActive: false
            });
        }

        this.menuClick = false;
    }

    onToggleMenu(event) {
        this.menuClick = true;

        if (this.isDesktop()) {
            if (this.state.layoutMode === 'overlay') {
                this.setState({
                    overlayMenuActive: !this.state.overlayMenuActive
                });
            }
            else if (this.state.layoutMode === 'static') {
                this.setState({
                    staticMenuInactive: !this.state.staticMenuInactive
                });
            }
        }
        else {
            const mobileMenuActive = this.state.mobileMenuActive;
            this.setState({
                mobileMenuActive: !mobileMenuActive
            });
        }

        event.preventDefault();
    }

    onSidebarClick(event) {
        this.menuClick = true;
    }

    onMenuItemClick(event) {
        if(!event.item.items) {
            this.setState({
                overlayMenuActive: false,
                mobileMenuActive: false
            })
        }
    }

    createMenu() {
        this.menu = [
            {label: 'Dashboard', icon: 'pi pi-fw pi-home', command: () => {window.location = '#/'}},
            {
                label: 'Menu Modes', icon: 'pi pi-fw pi-cog',
                items: [
                    {label: 'Static Menu', icon: 'pi pi-fw pi-bars',  command: () => this.setState({layoutMode: 'static'}) },
                    {label: 'Overlay Menu', icon: 'pi pi-fw pi-bars',  command: () => this.setState({layoutMode: 'overlay'}) }
                ]
            },
            {
                label: 'Menu Colors', icon: 'pi pi-fw pi-align-left',
                items: [
                    {label: 'Dark', icon: 'pi pi-fw pi-bars',  command: () => this.setState({layoutColorMode: 'dark'}) },
                    {label: 'Light', icon: 'pi pi-fw pi-bars',  command: () => this.setState({layoutColorMode: 'light'}) }
                ]
            },
            {
                label: 'Components', icon: 'pi pi-fw pi-globe', badge: '9',
                items: [
                    {label: 'Sample Page', icon: 'pi pi-fw pi-th-large', to: '/sample'},
                    {label: 'Forms', icon: 'pi pi-fw pi-file', to: '/forms'},
                    {label: 'Data', icon: 'pi pi-fw pi-table', to: '/data'},
                    {label: 'Panels', icon: 'pi pi-fw pi-list', to: '/panels'},
                    {label: 'Overlays', icon: 'pi pi-fw pi-clone', to: '/overlays'},
                    {label: 'Menus', icon: 'pi pi-fw pi-plus', to: '/menus'},
                    {label: 'Messages', icon: 'pi pi-fw pi-spinner',to: '/messages'},
                    {label: 'Charts', icon: 'pi pi-fw pi-chart-bar', to: '/charts'},
                    {label: 'Misc', icon: 'pi pi-fw pi-upload', to: '/misc'}
                ]
            },
            {
                label: 'Template Pages', icon: 'pi pi-fw pi-file',
                items: [
                    {label: 'Empty Page', icon: 'pi pi-fw pi-circle-off', to: '/empty'}
                ]
            },
            {
                label: 'Menu Hierarchy', icon: 'pi pi-fw pi-search',
                items: [
                    {
                        label: 'Submenu 1', icon: 'pi pi-fw pi-bookmark',
                        items: [
                            {
                                label: 'Submenu 1.1', icon: 'pi pi-fw pi-bookmark',
                                items: [
                                    {label: 'Submenu 1.1.1', icon: 'pi pi-fw pi-bookmark'},
                                    {label: 'Submenu 1.1.2', icon: 'pi pi-fw pi-bookmark'},
                                    {label: 'Submenu 1.1.3', icon: 'pi pi-fw pi-bookmark'},
                                ]
                            },
                            {
                                label: 'Submenu 1.2', icon: 'pi pi-fw pi-bookmark',
                                items: [
                                    {label: 'Submenu 1.2.1', icon: 'pi pi-fw pi-bookmark'},
                                    {label: 'Submenu 1.2.2', icon: 'pi pi-fw pi-bookmark'}
                                ]
                            },
                        ]
                    },
                    {
                        label: 'Submenu 2', icon: 'pi pi-fw pi-bookmark',
                        items: [
                            {
                                label: 'Submenu 2.1', icon: 'pi pi-fw pi-bookmark',
                                items: [
                                    {label: 'Submenu 2.1.1', icon: 'pi pi-fw pi-bookmark'},
                                    {label: 'Submenu 2.1.2', icon: 'pi pi-fw pi-bookmark'},
                                    {label: 'Submenu 2.1.3', icon: 'pi pi-fw pi-bookmark'},
                                ]
                            },
                            {
                                label: 'Submenu 2.2', icon: 'pi pi-fw pi-bookmark',
                                items: [
                                    {label: 'Submenu 2.2.1', icon: 'pi pi-fw pi-bookmark'},
                                    {label: 'Submenu 2.2.2', icon: 'pi pi-fw pi-bookmark'}
                                ]
                            }
                        ]
                    }
                ]
            },
            {label: 'Documentation', icon: 'pi pi-fw pi-question', command: () => {window.location = "#/documentation"}},
            {label: 'View Source', icon: 'pi pi-fw pi-search', command: () => {window.location = "https://github.com/primefaces/sigma"}}
        ];
    }

    addClass(element, className) {
        if (element.classList)
            element.classList.add(className);
        else
            element.className += ' ' + className;
    }

    removeClass(element, className) {
        if (element.classList)
            element.classList.remove(className);
        else
            element.className = element.className.replace(new RegExp('(^|\\b)' + className.split(' ').join('|') + '(\\b|$)', 'gi'), ' ');
    }
}

export default App;

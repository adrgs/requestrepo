import axios from 'axios';

export class Utils {

    static siteUrl = process.env.DOMAIN || "requestrepo.com";
    static apiUrl = "";
    static requestsEndpoint = "/api/get_requests";
    static subdomainEndpoint = "/api/get_token";
    static deleteRequestEndpoint = "/api/delete_request";
    static fileEndpoint = "/api/get_file";
    static updateFileEndpoint = "/api/update_file";
    static DNSRecordsEndpoint = "/api/get_dns_records";
    static updateDNSRecordsEndpoint = "/api/update_dns_records";
    static subdomain = "";

    static async getRequests(timestamp) {
        let reqUrl = this.apiUrl + this.requestsEndpoint;
        if (timestamp != undefined) {
            reqUrl += "?t=" + timestamp;
        }
        let res = await axios.get(reqUrl, { withCredentials: true });
        return res.data;
    }

    static async getDNSRecords() {
        let reqUrl = this.apiUrl + this.DNSRecordsEndpoint;
        let res = await axios.get(reqUrl, { withCredentials: true });
        return res.data;
    }

    static async updateDNSRecords(data) {
        let reqUrl = this.apiUrl + this.updateDNSRecordsEndpoint;
        let res = await axios.post(reqUrl, data, { withCredentials: true });
        return res.data;
    }

    static async getFile() {
        let reqUrl = this.apiUrl + this.fileEndpoint;
        let res = await axios.get(reqUrl, { withCredentials: true });
        return res.data;
    }

    static async updateFile(data) {
        let reqUrl = this.apiUrl + this.updateFileEndpoint;
        let res = await axios.post(reqUrl, data, { withCredentials: true });
        return res.data;
    }

    static getCookie(name) {
        let decodedCookie = decodeURIComponent(document.cookie);
        let ca = decodedCookie.split(';');
        for (let i = 0; i < ca.length; i++) {
            let c = ca[i];
            while (c.charAt(0) == ' ') {
                c = c.substring(1);
            }
            if (c.indexOf(name) == 0) {
                return c.substring(name.length, c.length);
            }
        }
        return "";
    };

    static getUserURL() {
        return this.subdomain + "." + this.siteUrl;
    }

    static userHasSubdomain() {
        if (this.subdomain === "") {
            let cookie = this.getCookie("token");
            if (cookie === "") return false;
            cookie = cookie.split('.');
            if (cookie.length < 2) return false;
            cookie = cookie[1];
            let jsonToken = JSON.parse(atob(cookie));
            if (jsonToken['subdomain'] !== undefined) {
                this.subdomain = jsonToken['subdomain'];
            }
        }
        return (this.subdomain !== "");
    }
    static getRandomSubdomain() {
        let reqUrl = this.apiUrl + this.subdomainEndpoint;
        return axios.post(reqUrl, null, { withCredentials: true }).then(function (response) {
            localStorage.clear();
            window.location.reload();
        });
    }

    static deleteRequest(id, type) {
        let reqUrl = this.apiUrl + this.deleteRequestEndpoint;
        return axios.post(reqUrl, { "id": id, "type": type }, { withCredentials: true });
    }

}
import axios from "axios";

export class Utils {
  static siteUrl = process.env.DOMAIN || "requestrepo.com";
  static apiUrl = "";
  static requestsEndpoint = "/api/get_requests";
  static subdomainEndpoint = "/api/get_token";
  static deleteRequestEndpoint = "/api/delete_request";
  static deleteAllEndpoint = "/api/delete_all";
  static fileEndpoint = "/api/get_file";
  static updateFileEndpoint = "/api/update_file";
  static DNSRecordsEndpoint = "/api/get_dns";
  static updateDNSRecordsEndpoint = "/api/update_dns";
  static subdomain = "";

  static async getDNSRecords() {
    let reqUrl = this.apiUrl + this.DNSRecordsEndpoint;
    let res = await axios.get(reqUrl, { params: { token: localStorage.getItem("token") } });
    return res.data;
  }

  static async updateDNSRecords(data) {
    let reqUrl = this.apiUrl + this.updateDNSRecordsEndpoint;
    let res = await axios.post(reqUrl, data, { params: { token: localStorage.getItem("token") } });
    return res.data;
  }

  static async getFile() {
    let reqUrl = this.apiUrl + this.fileEndpoint;
    let res = await axios.get(reqUrl, { params: { token: localStorage.getItem("token") } });
    return res.data;
  }

  static async updateFile(data) {
    let reqUrl = this.apiUrl + this.updateFileEndpoint;
    let res = await axios.post(reqUrl, data, { params: { token: localStorage.getItem("token") } });
    return res.data;
  }

  static getUserURL() {
    return this.subdomain + "." + this.siteUrl;
  }

  static userHasSubdomain() {
    if (this.subdomain === "") {
      let cookie = localStorage.getItem("token");
      if (!cookie) return false;
      cookie = cookie.split(".");
      if (cookie.length < 2) return false;
      cookie = cookie[1];
      let jsonToken = JSON.parse(Utils.base64DecodeUnicode(cookie));
      if (jsonToken["subdomain"] !== undefined) {
        this.subdomain = jsonToken["subdomain"];
      }
    }
    return this.subdomain !== "";
  }
  static getRandomSubdomain() {
    let reqUrl = this.apiUrl + this.subdomainEndpoint;
    return axios.post(reqUrl, null, { withCredentials: true }).then(function (response) {
      localStorage.clear();
      localStorage.setItem("token", response.data.token);
      window.location.reload();
    });

  }

  static deleteRequest(id) {
    let reqUrl = this.apiUrl + this.deleteRequestEndpoint;
    return axios.post(reqUrl, { id: id }, { params: { token: localStorage.getItem("token") } });
  }

  static deleteAll(id, type) {
    let reqUrl = this.apiUrl + this.deleteAllEndpoint;
    return axios.post(reqUrl, null, { params: { token: localStorage.getItem("token") } });
  }

  static base64EncodeUnicode(str) {
    // Convert each character to Latin1 or URL-encoded
    var latin1OrEncodedStr = Array.from(str).map(function (c) {
      var code = c.charCodeAt(0);
      // Latin1 characters are in the range of 0 to 255
      return (code >= 0 && code <= 255) ? c : encodeURIComponent(c);
    }).join('');

    // Base64 encode the modified string
    return btoa(latin1OrEncodedStr);
  }

  static base64DecodeUnicode(str) {
    // Decode from base64
    var binaryString = atob(str);

    try {
      // Convert binary string to a percent-encoded string
      var percentEncodedStr = binaryString.split('').map(function (c) {
        return '%' + ('00' + c.charCodeAt(0).toString(16)).slice(-2);
      }).join('');

      // Attempt to decode as UTF-8
      return decodeURIComponent(percentEncodedStr);
    } catch (e) {
      // If UTF-8 decoding fails, return the binary string (Latin1)
      return binaryString;
    }
  }
}

import axios from "axios";
import { Base64 } from "js-base64";

export class Utils {
  static siteUrl = import.meta.env.DEV ? "localhost:21337" : import.meta.env.VITE_DOMAIN || "requestrepo.com";
  static domain = this.siteUrl.split(":")[0];
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
    let res = await axios.get(reqUrl, {
      params: { token: localStorage.getItem("token") },
    });
    return res.data;
  }

  static async updateDNSRecords(data) {
    let reqUrl = this.apiUrl + this.updateDNSRecordsEndpoint;
    let res = await axios.post(reqUrl, data, {
      params: { token: localStorage.getItem("token") },
    });
    return res.data;
  }

  static async getFile() {
    let reqUrl = this.apiUrl + this.fileEndpoint;
    let res = await axios.get(reqUrl, {
      params: { token: localStorage.getItem("token") },
    });
    return res.data;
  }

  static async updateFile(data) {
    let reqUrl = this.apiUrl + this.updateFileEndpoint;
    let res = await axios.post(reqUrl, data, {
      params: { token: localStorage.getItem("token") },
    });
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
      let jsonToken = JSON.parse(Utils.base64Decode(cookie));
      if (jsonToken["subdomain"] !== undefined) {
        this.subdomain = jsonToken["subdomain"];
      }
    }
    return this.subdomain !== "";
  }
  static getRandomSubdomain() {
    let reqUrl = this.apiUrl + this.subdomainEndpoint;
    return axios.post(reqUrl, null, { withCredentials: true }).then(function (response) {
      let theme = localStorage.getItem("theme");
      localStorage.clear();
      localStorage.setItem("token", response.data.token);
      if (theme) localStorage.setItem("theme", theme);
      window.location.reload();
    });
  }

  static deleteRequest(id) {
    let reqUrl = this.apiUrl + this.deleteRequestEndpoint;
    return axios.post(reqUrl, { id: id }, { params: { token: localStorage.getItem("token") } });
  }

  static deleteAll() {
    let reqUrl = this.apiUrl + this.deleteAllEndpoint;
    return axios.post(reqUrl, null, {
      params: { token: localStorage.getItem("token") },
    });
  }

  static isValidUTF8(input) {
    try {
      let buf = new ArrayBuffer(input.length);
      let bufView = new Uint8Array(buf);
      for (var i=0, strLen=input.length; i<strLen; i++) {
        const val = input.charCodeAt(i);
        if (val > 255) return true;
        bufView[i] = val;
      }

      const decoder = new TextDecoder("utf-8", { fatal: true });
      decoder.decode(buf);
      return true;
    } catch (e) {
      return false;
    }
  }

  static base64Decode(str) {
    const raw = Base64.atob(str);
    if (Utils.isValidUTF8(raw)) {
      return Base64.decode(str);
    } else {
      return raw;
    }
  }

  static base64Encode(str) {
    if (Utils.isValidUTF8(str)) {
      return Base64.encode(str);
    } else {
      return Base64.btoa(str);
    }
  }

  static arrayBufferToString(buffer) {
    try {
      const decoder = new TextDecoder("utf-8", { fatal: true });
      return decoder.decode(buffer);
    } catch {
      const view = new Uint8Array(buffer);
      let output = '';
      for (let i = 0; i < buffer.byteLength; i++) {
        output += String.fromCharCode(view[i]);
      }
      return output;
    }
  }

  static initTheme() {
    if (localStorage.getItem("theme") !== "dark" && localStorage.getItem("theme") !== "light") {
      // get system theme
      if (window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches) {
        localStorage.setItem("theme", "dark");
      } else {
        localStorage.setItem("theme", "light");
      }
    }

    if (localStorage.getItem("theme") === "dark") {
      document.body.classList.add("dark");
    }
  }

  static toggleTheme() {
    if (document.body.classList.contains("dark")) {
      document.body.classList.remove("dark");
      localStorage.setItem("theme", "light");
    } else {
      document.body.classList.add("dark");
      localStorage.setItem("theme", "dark");
    }

    // Dispatch a custom event to notify components of the theme change
    window.dispatchEvent(new Event("themeChange"));
  }

  static getTheme() {
    this.initTheme();
    if (document.body.classList.contains("dark")) {
      return "dark";
    }
    return "light";
  }

  static isDarkTheme() {
    return this.getTheme() === "dark";
  }

  static isLightTheme() {
    return this.getTheme() === "light";
  }
}

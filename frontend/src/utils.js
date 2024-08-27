import axios from "axios";

export class Utils {
  static siteUrl = import.meta.env.DEV
    ? "localhost:21337"
    : import.meta.env.VITE_DOMAIN || "requestrepo.com";
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
    return await axios.post(reqUrl, data, {
      params: { token: localStorage.getItem("token") },
    });
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
    return axios
      .post(reqUrl, null, { withCredentials: true })
      .then(function (response) {
        let theme = localStorage.getItem("theme");
        localStorage.clear();
        localStorage.setItem("token", response.data.token);
        if (theme) localStorage.setItem("theme", theme);
        window.location.reload();
      });
  }

  static deleteRequest(id) {
    let reqUrl = this.apiUrl + this.deleteRequestEndpoint;
    return axios.post(
      reqUrl,
      { id: id },
      { params: { token: localStorage.getItem("token") } },
    );
  }

  static deleteAll() {
    let reqUrl = this.apiUrl + this.deleteAllEndpoint;
    return axios.post(reqUrl, null, {
      params: { token: localStorage.getItem("token") },
    });
  }

  static base64EncodeUnicode(str) {
    // Convert each character to Latin1 or URL-encoded
    var latin1OrEncodedStr = Array.from(str)
      .map(function (c) {
        var code = c.charCodeAt(0);
        // Latin1 characters are in the range of 0 to 255
        return code >= 0 && code <= 255 ? c : encodeURIComponent(c);
      })
      .join("");

    // Base64 encode the modified string
    return btoa(latin1OrEncodedStr);
  }

  static arrayBufferToBase64(buffer) {
    return btoa(this.arrayBufferToString(buffer));
  }

  static arrayBufferToString(buffer) {
    var binary = "";
    var bytes = new Uint8Array(buffer);
    var len = bytes.byteLength;
    for (var i = 0; i < len; i++) {
      binary += String.fromCharCode(bytes[i]);
    }
    return binary;
  }

  static base64EncodeRaw(str) {
    // Base64 encode the string
    return btoa(str);
  }

  static base64DecodeRaw(str) {
    // Base64 encode the string
    return atob(str);
  }

  static base64EncodeLatin1(str) {
    return this.arrayBufferToBase64(new TextEncoder().encode(str));
  }

  static base64DecodeUnicode(str) {
    // Decode from base64
    var binaryString = atob(str);

    try {
      // Convert binary string to a percent-encoded string
      var percentEncodedStr = binaryString
        .split("")
        .map(function (c) {
          return "%" + ("00" + c.charCodeAt(0).toString(16)).slice(-2);
        })
        .join("");

      // Attempt to decode as UTF-8
      return decodeURIComponent(percentEncodedStr);
    } catch (e) {
      // If UTF-8 decoding fails, return the binary string (Latin1)
      return binaryString;
    }
  }

  static initTheme() {
    if (
      localStorage.getItem("theme") !== "dark" &&
      localStorage.getItem("theme") !== "light"
    ) {
      // get system theme
      if (
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: dark)").matches
      ) {
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

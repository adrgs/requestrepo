import axios from "axios";
import { Base64 } from "js-base64";
import { toast } from "react-toastify";

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
  static pendingSessionPromise = null;
  static MAX_SESSIONS = 5;
  static sessions = JSON.parse(localStorage.getItem("sessions") || "[]");
  static selectedSessionIndex = parseInt(
    localStorage.getItem("selectedSessionIndex") || "0",
  );
  static getActiveSession() {
    try {
      const sessions = JSON.parse(localStorage.getItem("sessions") || "[]");
      if (sessions.length === 0) {
        this.selectedSessionIndex = 0;
        return null;
      }

      // Get and validate selectedSessionIndex
      let selectedIndex = parseInt(
        localStorage.getItem("selectedSessionIndex") || "0",
      );
      selectedIndex = Math.max(0, Math.min(selectedIndex, sessions.length - 1));

      // Update if index was invalid
      if (
        selectedIndex.toString() !==
        localStorage.getItem("selectedSessionIndex")
      ) {
        localStorage.setItem("selectedSessionIndex", selectedIndex.toString());
      }

      const session = sessions[selectedIndex];
      if (session) {
        this.subdomain = session.subdomain;
      }
      return session;
    } catch (error) {
      console.error("Error getting active session:", error);
      return null;
    }
  }

  static saveSessionsToStorage() {
    localStorage.setItem("sessions", JSON.stringify(this.sessions));
    localStorage.setItem(
      "selectedSessionIndex",
      this.selectedSessionIndex.toString(),
    );
  }

  static addSession(subdomain, token) {
    if (this.sessions.length >= this.MAX_SESSIONS) {
      throw new Error("Maximum number of sessions reached");
    }

    const newSession = {
      subdomain,
      token,
      createdAt: new Date().toISOString(),
      unseenRequests: 0,
    };

    this.sessions.push(newSession);
    this.selectedSessionIndex = this.sessions.length - 1;
    this.saveSessionsToStorage();
    return newSession;
  }

  static removeSession(index) {
    if (index < 0 || index >= this.sessions.length) {
      throw new Error("Invalid session index");
    }

    const removedSession = this.sessions[index];

    // Clean up all related data in localStorage
    const sessionPrefix = `requests_${removedSession.subdomain}`;
    const visitedKey = `visited_${removedSession.subdomain}`;
    const lastRequestKey = `lastRequest_${removedSession.subdomain}`;
    const dnsKey = `dns_${removedSession.subdomain}`;
    const filesKey = `files_${removedSession.subdomain}`;

    for (let i = localStorage.length - 1; i >= 0; i--) {
      const key = localStorage.key(i);
      if (
        key &&
        (key.startsWith(sessionPrefix) ||
          key === visitedKey ||
          key === lastRequestKey ||
          key === dnsKey ||
          key === filesKey)
      ) {
        localStorage.removeItem(key);
      }
    }

    // Store the removed session details for the event
    const removedSessionDetails = {
      subdomain: removedSession.subdomain,
      token: removedSession.token,
      index: index,
    };

    this.sessions.splice(index, 1);

    // Update selected index and ensure it's valid
    if (this.selectedSessionIndex >= this.sessions.length) {
      this.selectedSessionIndex = Math.max(0, this.sessions.length - 1);
    }

    // Update subdomain for the new active session
    if (this.sessions.length > 0) {
      this.subdomain = this.sessions[this.selectedSessionIndex].subdomain;
    } else {
      this.subdomain = "";
      this.selectedSessionIndex = 0;
    }

    this.saveSessionsToStorage();

    // Dispatch a more detailed event with comprehensive session state
    window.dispatchEvent(
      new CustomEvent("sessionChanged", {
        detail: {
          type: "remove",
          removedSession: removedSessionDetails,
          remainingSessions: this.sessions,
          activeSession: this.getActiveSession(),
          selectedIndex: this.selectedSessionIndex,
          totalSessions: this.sessions.length,
        },
      }),
    );

    return true;
  }

  static selectSession(index) {
    if (index < 0 || index >= this.sessions.length) {
      throw new Error("Invalid session index");
    }

    this.selectedSessionIndex = index;
    const session = this.sessions[index];
    this.subdomain = session.subdomain;
    this.saveSessionsToStorage();

    // Dispatch event with more complete session information
    window.dispatchEvent(
      new CustomEvent("sessionChanged", {
        detail: {
          type: "select",
          session,
          allSessions: this.sessions,
          selectedIndex: this.selectedSessionIndex,
        },
      }),
    );
  }

  static getSessionToken(subdomain = null) {
    try {
      // If no subdomain provided, use active session's subdomain
      if (!subdomain) {
        const activeSession = this.getActiveSession();
        if (activeSession) {
          return activeSession.token;
        }
        return null;
      }

      // Get sessions from localStorage
      const sessionsStr = localStorage.getItem("sessions");
      if (!sessionsStr) return null;

      const sessions = JSON.parse(sessionsStr);
      const session = sessions.find((s) => s.subdomain === subdomain);
      return session ? session.token : null;
    } catch (error) {
      console.error("Error getting session token:", error);
      return null;
    }
  }

  static setSessionToken(subdomain, token) {
    const existingIndex = this.sessions.findIndex(
      (s) => s.subdomain === subdomain,
    );
    if (existingIndex >= 0) {
      this.sessions[existingIndex].token = token;
    } else {
      try {
        this.addSession(subdomain, token);
      } catch (error) {
        toast.error(
          "Maximum number of sessions reached. Please close an existing session first.",
        );
        throw error;
      }
    }
    this.saveSessionsToStorage();
  }

  static getAllSessions() {
    return this.sessions;
  }

  static getAllSessionTokens() {
    return this.sessions
      .filter((session) => {
        // Validate token format
        try {
          JSON.parse(Utils.base64Decode(session.token.split(".")[1]));
          return true;
        } catch (error) {
          return false;
        }
      })
      .map((session) => ({
        subdomain: session.subdomain,
        token: session.token,
      }));
  }

  static updateSessionUnseenRequests(subdomain, count) {
    const session = this.sessions.find((s) => s.subdomain === subdomain);
    if (session) {
      session.unseenRequests = count;
      this.saveSessionsToStorage();
      return true;
    }
    return false;
  }

  static async getDNSRecords(subdomain = null) {
    let reqUrl = this.apiUrl + this.DNSRecordsEndpoint;
    let res = await axios.get(reqUrl, {
      params: { token: this.getSessionToken(subdomain) },
    });
    return res.data;
  }

  static async updateDNSRecords(data, subdomain = null) {
    let reqUrl = this.apiUrl + this.updateDNSRecordsEndpoint;
    let res = await axios.post(reqUrl, data, {
      params: { token: this.getSessionToken(subdomain) },
    });
    return res.data;
  }

  static async getFile() {
    let reqUrl = this.apiUrl + this.fileEndpoint;
    let res = await axios.get(reqUrl, {
      params: { token: this.getSessionToken(this.subdomain) },
    });
    return res.data;
  }

  static async updateFile(data) {
    let reqUrl = this.apiUrl + this.updateFileEndpoint;
    let res = await axios.post(reqUrl, data, {
      params: { token: this.getSessionToken(this.subdomain) },
    });
    return res.data;
  }

  static getUserURL() {
    return this.subdomain + "." + this.siteUrl;
  }

  static userHasSubdomain() {
    try {
      const sessions = JSON.parse(localStorage.getItem("sessions") || "[]");
      if (sessions.length > 0) {
        const activeSession = this.getActiveSession();
        if (activeSession) {
          this.subdomain = activeSession.subdomain;
          return true;
        }
      }
      return false;
    } catch (error) {
      console.error("Error in userHasSubdomain:", error);
      return false;
    }
  }

  static async getRandomSubdomain() {
    let reqUrl = this.apiUrl + this.subdomainEndpoint;

    // If there's already a pending session creation, return that promise
    if (this.pendingSessionPromise) {
      return this.pendingSessionPromise;
    }

    // Create new promise for session creation
    this.pendingSessionPromise = new Promise(async (resolve, reject) => {
      try {
        // Check if we've reached the maximum number of sessions
        const sessions = JSON.parse(localStorage.getItem("sessions") || "[]");
        if (sessions.length >= this.MAX_SESSIONS) {
          throw new Error("Maximum number of sessions reached");
        }

        const response = await axios.post(reqUrl, null, {
          withCredentials: true,
        });
        const token = response.data.token;

        if (!token) {
          throw new Error("No token received from server");
        }

        const tokenData = JSON.parse(Utils.base64Decode(token.split(".")[1]));
        const subdomain = tokenData.subdomain;

        if (!subdomain) {
          throw new Error("Invalid token format: no subdomain found");
        }

        resolve({ subdomain, token });
      } catch (error) {
        console.error("Error in getRandomSubdomain:", error);
        reject(
          new Error(
            error.response?.data?.message ||
              error.message ||
              "Failed to initialize session",
          ),
        );
      }
    }).finally(() => {
      this.pendingSessionPromise = null;
    });

    return this.pendingSessionPromise;
  }

  static deleteRequest(id) {
    let reqUrl = this.apiUrl + this.deleteRequestEndpoint;
    return axios.post(
      reqUrl,
      { id: id },
      { params: { token: this.getSessionToken(this.subdomain) } },
    );
  }

  static deleteAll() {
    let reqUrl = this.apiUrl + this.deleteAllEndpoint;
    return axios.post(reqUrl, null, {
      params: { token: this.getSessionToken(this.subdomain) },
    });
  }

  static isValidUTF8(input) {
    try {
      let buf = new ArrayBuffer(input.length);
      let bufView = new Uint8Array(buf);
      for (var i = 0, strLen = input.length; i < strLen; i++) {
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
      let output = "";
      for (let i = 0; i < buffer.byteLength; i++) {
        output += String.fromCharCode(view[i]);
      }
      return output;
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
    const body = document.body;
    const isDark = !body.classList.contains("dark");
    body.classList.toggle("dark");
    localStorage.setItem("theme", isDark ? "dark" : "light");

    window.dispatchEvent(new Event("themeChange"));

    // Return the new theme state immediately
    return isDark;
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

  static async getFiles(subdomain = null) {
    const token = this.getSessionToken(subdomain);
    const response = await fetch(`/api/files?token=${token}`);
    if (!response.ok) throw new Error("Failed to fetch files");
    return response.json();
  }

  static async updateFiles(files, subdomain = null) {
    const token = this.getSessionToken(subdomain);
    const response = await fetch(`/api/files?token=${token}`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify(files),
    });
    if (!response.ok) throw new Error("Failed to update files");
    return response.json();
  }

  static cleanupSessionStorage(subdomain) {
    // Clean up all known keys
    const keysToRemove = [
      `token_${subdomain}`,
      `visited_${subdomain}`,
      `lastSelectedRequest_${subdomain}`,
      `lastDeletedRequest_${subdomain}`,
      `dns_${subdomain}`,
      `files_${subdomain}`,
    ];

    keysToRemove.forEach((key) => {
      localStorage.removeItem(key);
    });

    // Clean up any other keys that might contain the subdomain
    for (let i = localStorage.length - 1; i >= 0; i--) {
      const key = localStorage.key(i);
      if (key && key.includes(subdomain)) {
        localStorage.removeItem(key);
      }
    }

    // Update sessions array
    try {
      const sessionsStr = localStorage.getItem("sessions");
      if (sessionsStr) {
        const sessions = JSON.parse(sessionsStr);
        const updatedSessions = sessions.filter(
          (s) => s.subdomain !== subdomain,
        );
        localStorage.setItem("sessions", JSON.stringify(updatedSessions));

        // Update selectedSessionIndex if needed
        let selectedIndex = parseInt(
          localStorage.getItem("selectedSessionIndex") || "0",
        );
        if (selectedIndex >= updatedSessions.length) {
          selectedIndex = Math.max(0, updatedSessions.length - 1);
          localStorage.setItem(
            "selectedSessionIndex",
            selectedIndex.toString(),
          );
        }
      }
    } catch (error) {
      console.error("Error updating sessions in localStorage:", error);
    }
  }
}

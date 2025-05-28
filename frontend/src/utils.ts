import axios from "axios";
import { Base64 } from "js-base64";
import { toast } from "react-toastify";
import { ApiResponse, DNSUpdateResponse, FileResponse, Request } from "./types/app-types";

export interface Session {
  subdomain: string;
  token: string;
  createdAt: string;
  unseenRequests: number;
}

interface SessionToken {
  subdomain: string;
  token: string;
}

interface RemovedSessionDetails {
  subdomain: string;
  token: string;
  index: number;
}

export interface DNSRecord {
  domain: string;
  type: number | string;
  value: string;
  subdomain?: string;
  name?: string;
  content?: string;
  ttl?: number;
}

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
  static getRequestEndpoint = "/api/get_request";
  static subdomain = "";
  static pendingSessionPromise: Promise<{
    subdomain: string;
    token: string;
  }> | null = null;
  static MAX_SESSIONS = 5;
  static sessions: Session[] = JSON.parse(
    localStorage.getItem("sessions") || "[]",
  );
  static selectedSessionIndex = parseInt(
    localStorage.getItem("selectedSessionIndex") || "0",
  );
  static getActiveSession(): Session | null {
    try {
      const activeSessionSubdomain = sessionStorage.getItem(
        "activeSessionSubdomain",
      );
      if (activeSessionSubdomain) {
        const allSessions = this.getAllSessions();
        const activeSession = allSessions.find(
          (s) => s.subdomain === activeSessionSubdomain,
        );
        if (activeSession) {
          return activeSession;
        }
      }

      const sessionsStr = localStorage.getItem("sessions");
      if (!sessionsStr) return null;

      const sessions: Session[] = JSON.parse(sessionsStr);
      if (sessions.length === 0) return null;

      const selectedIndex = parseInt(
        localStorage.getItem("selectedSessionIndex") || "0",
      );
      const validIndex = Math.max(
        0,
        Math.min(selectedIndex, sessions.length - 1),
      );

      const selectedSession = sessions[validIndex];
      if (selectedSession) {
        sessionStorage.setItem(
          "activeSessionSubdomain",
          selectedSession.subdomain,
        );
      }

      return sessions[validIndex];
    } catch (error) {
      console.error("Error getting active session", error);
      return null;
    }
  }

  static saveSessionsToStorage(): void {
    localStorage.setItem("sessions", JSON.stringify(this.sessions));
    localStorage.setItem(
      "selectedSessionIndex",
      this.selectedSessionIndex.toString(),
    );
  }

  static addSession(subdomain: string, token: string): Session {
    if (this.sessions.length >= this.MAX_SESSIONS) {
      throw new Error("Maximum number of sessions reached");
    }

    const newSession: Session = {
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

  static removeSession(index: number): boolean {
    if (index < 0 || index >= this.sessions.length) {
      throw new Error("Invalid session index");
    }

    const removedSession = this.sessions[index];

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

    const removedSessionDetails: RemovedSessionDetails = {
      subdomain: removedSession.subdomain,
      token: removedSession.token,
      index: index,
    };

    this.sessions.splice(index, 1);

    if (this.selectedSessionIndex >= this.sessions.length) {
      this.selectedSessionIndex = Math.max(0, this.sessions.length - 1);
    }

    if (this.sessions.length > 0) {
      this.subdomain = this.sessions[this.selectedSessionIndex].subdomain;
    } else {
      this.subdomain = "";
      this.selectedSessionIndex = 0;
    }

    this.saveSessionsToStorage();

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

  static selectSession(index: number): void {
    if (index < 0 || index >= this.sessions.length) {
      throw new Error("Invalid session index");
    }

    this.selectedSessionIndex = index;
    const session = this.sessions[index];
    this.subdomain = session.subdomain;
    this.saveSessionsToStorage();

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

  static getSessionToken(subdomain: string | null = null): string | null {
    try {
      if (!subdomain) {
        const activeSession = this.getActiveSession();
        if (activeSession) {
          return activeSession.token;
        }
        return null;
      }

      const sessionsStr = localStorage.getItem("sessions");
      if (!sessionsStr) return null;

      const sessions: Session[] = JSON.parse(sessionsStr);
      const session = sessions.find((s) => s.subdomain === subdomain);
      return session ? session.token : null;
    } catch (error) {
      console.error("Error getting session token:", error);
      return null;
    }
  }

  static setSessionToken(subdomain: string, token: string): void {
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
          Utils.toastOptions,
        );
        throw error;
      }
    }
    this.saveSessionsToStorage();
  }

  static getAllSessions(): Session[] {
    return this.sessions;
  }

  static getAllSessionTokens(): SessionToken[] {
    return this.sessions
      .filter((session) => {
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

  static updateSessionUnseenRequests(
    subdomain: string,
    count: number,
  ): boolean {
    const session = this.sessions.find((s) => s.subdomain === subdomain);
    if (session) {
      session.unseenRequests = count;
      this.saveSessionsToStorage();
      return true;
    }
    return false;
  }

  static async getDNSRecords(
    subdomain: string | null = null,
  ): Promise<DNSRecord[]> {
    const reqUrl = this.apiUrl + this.DNSRecordsEndpoint;
    const res = await axios.get(reqUrl, {
      params: { token: this.getSessionToken(subdomain) },
    });
    return res.data;
  }

  static async updateDNSRecords(
    data: { records: DNSRecord[] },
    subdomain: string | null = null,
  ): Promise<DNSUpdateResponse> {
    const reqUrl = this.apiUrl + this.updateDNSRecordsEndpoint;
    const res = await axios.post(reqUrl, data, {
      params: { token: this.getSessionToken(subdomain) },
    });
    return res.data;
  }

  static async getFile(): Promise<FileResponse> {
    const reqUrl = this.apiUrl + this.fileEndpoint;
    const res = await axios.get(reqUrl, {
      params: { token: this.getSessionToken(this.subdomain) },
    });
    return res.data;
  }

  static async getRequest(
    id: string,
    subdomain: string,
  ): Promise<ApiResponse<Request>> {
    const reqUrl = this.apiUrl + this.getRequestEndpoint;
    const res = await axios.get(reqUrl, {
      params: { id, subdomain },
    });
    return res.data;
  }

  static async updateFile(
    data: FileResponse,
  ): Promise<ApiResponse<FileResponse>> {
    const reqUrl = this.apiUrl + this.updateFileEndpoint;
    const res = await axios.post(reqUrl, data, {
      params: { token: this.getSessionToken(this.subdomain) },
    });
    return res.data;
  }

  static async fetchResponse(subdomain: string): Promise<{
    raw: string;
    headers: Array<{ key: string; value: string }>;
    status_code: number;
    fetched: boolean;
  }> {
    try {
      const token = this.getSessionToken(subdomain);
      const response = await fetch(`/api/response?token=${token}`);
      if (!response.ok) throw new Error("Failed to fetch response");
      const data = await response.json();
      return {
        ...data,
        fetched: true,
      };
    } catch (error) {
      console.error("Error fetching response:", error);
      throw error;
    }
  }

  static async updateResponse(
    subdomain: string,
    data: {
      raw?: string;
      headers?: Array<{ key: string; value: string }>;
      status_code?: number;
    },
  ): Promise<ApiResponse<{
    raw?: string;
    headers?: Array<{ key: string; value: string }>;
    status_code?: number;
  }>> {
    try {
      const token = this.getSessionToken(subdomain);
      const response = await fetch(`/api/response?token=${token}`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(data),
      });
      if (!response.ok) throw new Error("Failed to update response");
      return response.json();
    } catch (error) {
      console.error("Error updating response:", error);
      throw error;
    }
  }

  static getUserURL(): string {
    return this.subdomain + "." + this.siteUrl;
  }

  static userHasSubdomain(): boolean {
    try {
      const sessions: Session[] = JSON.parse(
        localStorage.getItem("sessions") || "[]",
      );
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

  static async getRandomSubdomain(): Promise<{
    subdomain: string;
    token: string;
  }> {
    const reqUrl = this.apiUrl + this.subdomainEndpoint;

    if (this.pendingSessionPromise) {
      return this.pendingSessionPromise;
    }

    this.pendingSessionPromise = new Promise((resolve, reject) => {
      const sessions: Session[] = JSON.parse(
        localStorage.getItem("sessions") || "[]",
      );
      if (sessions.length >= this.MAX_SESSIONS) {
        reject(new Error("Maximum number of sessions reached"));
        return;
      }

      axios
        .post(reqUrl, null, {
          headers: {
            "Content-Type": "application/json",
          },
        })
        .then((response) => {
          const { subdomain, token } = response.data;
          resolve({ subdomain, token });
        })
        .catch((error) => {
          reject(error);
        })
        .finally(() => {
          this.pendingSessionPromise = null;
        });
    });

    return this.pendingSessionPromise;
  }

  static deleteRequest(
    id: string,
    subdomain: string | null = null,
  ): Promise<ApiResponse<{ success: boolean }>> {
    const reqUrl = this.apiUrl + this.deleteRequestEndpoint;
    return axios.post(
      reqUrl,
      { id: id },
      { params: { token: this.getSessionToken(subdomain) } },
    );
  }

  static deleteAll(
    subdomain: string | null = null,
  ): Promise<ApiResponse<{ success: boolean }>> {
    const reqUrl = this.apiUrl + this.deleteAllEndpoint;
    return axios.post(reqUrl, null, {
      params: { token: this.getSessionToken(subdomain) },
    });
  }

  static isValidUTF8(input: string): boolean {
    try {
      const buf = new ArrayBuffer(input.length);
      const bufView = new Uint8Array(buf);
      for (let i = 0, strLen = input.length; i < strLen; i++) {
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

  static base64Decode(str: string): string {
    const raw = Base64.atob(str);
    if (Utils.isValidUTF8(raw)) {
      return Base64.decode(str);
    } else {
      return raw;
    }
  }

  static base64Encode(str: string): string {
    if (Utils.isValidUTF8(str)) {
      return Base64.encode(str);
    } else {
      return Base64.btoa(str);
    }
  }

  static arrayBufferToString(buffer: ArrayBuffer): string {
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

  static initTheme(): void {
    if (
      localStorage.getItem("theme") !== "dark" &&
      localStorage.getItem("theme") !== "light"
    ) {
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

  static toggleTheme(): boolean {
    const body = document.body;
    const isDark = !body.classList.contains("dark");
    body.classList.toggle("dark");
    localStorage.setItem("theme", isDark ? "dark" : "light");

    window.dispatchEvent(new Event("themeChange"));

    return isDark;
  }

  static getTheme(): string {
    this.initTheme();
    if (document.body.classList.contains("dark")) {
      return "dark";
    }
    return "light";
  }

  static isDarkTheme(): boolean {
    return this.getTheme() === "dark";
  }

  static isLightTheme(): boolean {
    return this.getTheme() === "light";
  }

  static async getFiles(
    subdomain: string | null = null,
  ): Promise<ApiResponse<FileResponse>> {
    const token = this.getSessionToken(subdomain);
    const response = await fetch(`/api/files?token=${token}`);
    if (!response.ok) throw new Error("Failed to fetch files");
    return response.json();
  }

  static async updateFiles(
    files: FileResponse,
    subdomain: string | null = null,
  ): Promise<ApiResponse<FileResponse>> {
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

  static cleanupSessionStorage(subdomain: string): void {
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

    for (let i = localStorage.length - 1; i >= 0; i--) {
      const key = localStorage.key(i);
      if (key && key.includes(subdomain)) {
        localStorage.removeItem(key);
      }
    }

    try {
      const sessionsStr = localStorage.getItem("sessions");
      if (sessionsStr) {
        const sessions: Session[] = JSON.parse(sessionsStr);
        const updatedSessions = sessions.filter(
          (s) => s.subdomain !== subdomain,
        );
        localStorage.setItem("sessions", JSON.stringify(updatedSessions));

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

  static toastOptions = {
    position: "bottom-center",
    autoClose: 2000,
    hideProgressBar: false,
    closeOnClick: true,
    pauseOnHover: true,
    draggable: true,
    dark: Utils.isDarkTheme(),
  };
}

/**
 * @typedef {Object} Session
 * @property {string} subdomain
 * @property {string} token
 * @property {string} createdAt
 * @property {number} unseenRequests
 */

/**
 * @typedef {Object} DNSRecord
 * @property {string} domain
 * @property {number} type
 * @property {string} value
 */

const getTestID = jest.fn((id) => `test-id-${id}`);

const Utils = {
  siteUrl: "http://localhost:3000",
  apiUrl: "http://localhost:8000",
  domain: "example.com",
  MAX_SESSIONS: 10,
  toastOptions: {},
  sessions: [],
  selectedSessionIndex: 0,
  getTestID,

  /**
   * @param {string} subdomain
   * @param {string} token
   * @returns {Session}
   */
  addSession: jest.fn((subdomain, token) => {
    if (Utils.sessions.length >= Utils.MAX_SESSIONS) {
      throw new Error("Maximum number of sessions reached");
    }

    const session = {
      subdomain,
      token,
      createdAt: new Date().toISOString(),
      unseenRequests: 0,
    };

    Utils.sessions.push(session);
    Utils.selectedSessionIndex = Utils.sessions.length - 1;
    return session;
  }),

  /**
   * @param {number} index
   * @returns {boolean}
   */
  removeSession: jest.fn((index) => {
    if (index >= 0 && index < Utils.sessions.length) {
      Utils.sessions.splice(index, 1);
      if (Utils.selectedSessionIndex >= Utils.sessions.length) {
        Utils.selectedSessionIndex = Math.max(0, Utils.sessions.length - 1);
      }
      return true;
    }
    return false;
  }),

  /**
   * @returns {Session|null}
   */
  getActiveSession: jest.fn(() => {
    return Utils.sessions[Utils.selectedSessionIndex] || null;
  }),

  /**
   * @param {string|null} [subdomain=null]
   * @returns {string|null}
   */
  getSessionToken: jest.fn((subdomain = null) => {
    if (!subdomain) {
      const activeSession = Utils.getActiveSession();
      return activeSession ? activeSession.token : null;
    }

    const session = Utils.sessions.find((s) => s.subdomain === subdomain);
    return session ? session.token : null;
  }),

  /**
   * @param {string|null} [_subdomain=null]
   * @returns {Promise<DNSRecord[]>}
   */
  getDNSRecords: jest.fn((_subdomain = null) => {
    return Promise.resolve([{ domain: "", type: 0, value: "1.2.3.4" }]);
  }),

  /**
   * @param {Object} _data
   * @param {string|null} [_subdomain=null]
   * @returns {Promise<Object>}
   */
  updateDNSRecords: jest.fn((_data, _subdomain = null) => {
    return Promise.resolve({ msg: "Updated DNS records" });
  }),

  /**
   * @param {string} str
   * @returns {string}
   */
  base64Encode: jest.fn((str) => btoa(str)),

  /**
   * @param {string} str
   * @returns {string}
   */
  base64Decode: jest.fn((str) => atob(str)),

  /**
   * @param {ArrayBuffer} buffer
   * @returns {string}
   */
  arrayBufferToString: jest.fn((buffer) => {
    const bytes = new Uint8Array(buffer);
    return String.fromCharCode.apply(null, Array.from(bytes));
  }),

  /**
   * @returns {boolean}
   */
  isDarkTheme: jest.fn(() => {
    return false;
  }),

  /**
   * @returns {boolean}
   */
  toggleTheme: jest.fn(() => {
    const isDark = !document.body.classList.contains("dark");

    if (isDark) {
      document.body.classList.add("dark");
      window.localStorage.setItem("theme", "dark");
    } else {
      document.body.classList.remove("dark");
      window.localStorage.setItem("theme", "light");
    }

    window.dispatchEvent(new Event("themechange"));
    return isDark;
  }),

  /**
   * @param {string} _input
   * @returns {boolean}
   */
  isValidUTF8: jest.fn((_input) => true),
};

module.exports = { Utils, getTestID };

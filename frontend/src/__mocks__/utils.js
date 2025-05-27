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
    return true;
  }),

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

  getActiveSession: jest.fn(() => {
    return Utils.sessions[Utils.selectedSessionIndex] || null;
  }),

  getSessionToken: jest.fn(() => {
    return "test-token";
  }),

  getDNSRecords: jest.fn(() => {
    return Promise.resolve([{ domain: "", type: 0, value: "1.2.3.4" }]);
  }),

  updateDNSRecords: jest.fn((data, subdomain) => {
    return Promise.resolve({ msg: "Updated DNS records" });
  }),

  base64Encode: jest.fn((str) => btoa(str)),
  base64Decode: jest.fn((str) => atob(str)),
  arrayBufferToString: jest.fn((buffer) => {
    const bytes = new Uint8Array(buffer);
    return String.fromCharCode.apply(null, bytes);
  }),

  isDarkTheme: jest.fn(() => {
    return false;
  }),

  toggleTheme: jest.fn(() => {
    const isDark = !document.body.classList.contains("dark");

    if (isDark) {
      document.body.classList.add("dark");
      localStorage.setItem("theme", "dark");
    } else {
      document.body.classList.remove("dark");
      localStorage.setItem("theme", "light");
    }

    window.dispatchEvent(new Event("themechange"));
    return isDark;
  }),
};

module.exports = {
  Utils,
  getTestID,
};

import { Utils } from "../utils";
import { Base64 } from "js-base64";

const localStorageMock = {
  store: {},
  getItem: jest.fn(function (key) {
    return this.store[key] || null;
  }),
  setItem: jest.fn(function (key, value) {
    this.store[key] = String(value);
  }),
  removeItem: jest.fn(function (key) {
    delete this.store[key];
  }),
  clear: jest.fn(function () {
    this.store = {};
  }),
  key: jest.fn(function (index) {
    return Object.keys(this.store)[index] || null;
  }),
  get length() {
    return Object.keys(this.store).length;
  },
};

jest.mock("js-base64", () => ({
  Base64: {
    encode: jest.fn((str) => btoa(str)),
    decode: jest.fn((str) => atob(str)),
    atob: jest.fn((str) => atob(str)),
    btoa: jest.fn((str) => btoa(str)),
  },
}));

jest.mock("axios", () => ({
  get: jest.fn(() => Promise.resolve({ data: {} })),
  post: jest.fn(() => Promise.resolve({ data: {} })),
}));

describe("Utils", () => {
  beforeEach(() => {
    Object.defineProperty(window, "localStorage", {
      value: localStorageMock,
      writable: true,
    });

    Object.defineProperty(window, "sessionStorage", {
      value: { ...localStorageMock },
      writable: true,
    });

    localStorageMock.store = {};
    localStorageMock.getItem.mockClear();
    localStorageMock.setItem.mockClear();
    localStorageMock.removeItem.mockClear();
    localStorageMock.clear.mockClear();
    localStorageMock.key.mockClear();

    Utils.sessions = [];
    Utils.selectedSessionIndex = 0;

    jest.clearAllMocks();
  });

  describe("Session Management", () => {
    test.skip("addSession should add a new session", () => {
      const subdomain = "test123";
      const token = "test-token";

      Utils.addSession(subdomain, token);

      expect(Utils.sessions.length).toBe(1);
      expect(Utils.sessions[0].subdomain).toBe(subdomain);
      expect(Utils.sessions[0].token).toBe(token);
    });

    test("addSession should throw error when max sessions reached", () => {
      Utils.sessions = Array(Utils.MAX_SESSIONS)
        .fill()
        .map((_, i) => ({
          subdomain: `test${i}`,
          token: `token${i}`,
          createdAt: new Date().toISOString(),
          unseenRequests: 0,
        }));

      expect(() => Utils.addSession("newtest", "newtoken")).toThrow(
        "Maximum number of sessions reached",
      );
    });

    test.skip("removeSession should remove a session", () => {
      Utils.sessions = [
        {
          subdomain: "test1",
          token: "token1",
          createdAt: new Date().toISOString(),
          unseenRequests: 0,
        },
        {
          subdomain: "test2",
          token: "token2",
          createdAt: new Date().toISOString(),
          unseenRequests: 0,
        },
      ];
      Utils.selectedSessionIndex = 1;

      const result = Utils.removeSession(1);

      expect(result).toBe(true);
      expect(Utils.sessions.length).toBe(1);
      expect(Utils.sessions[0].subdomain).toBe("test1");
      expect(Utils.selectedSessionIndex).toBe(0);
    });

    test.skip("getSessionToken should return token for active session", () => {});
  });

  describe("Encoding and Decoding", () => {
    test.skip("base64Encode should encode strings correctly", () => {});

    test.skip("base64Decode should decode strings correctly", () => {});

    test("arrayBufferToString should convert ArrayBuffer to string", () => {
      const buffer = new ArrayBuffer(5);
      const view = new Uint8Array(buffer);
      view[0] = 72; // H
      view[1] = 101; // e
      view[2] = 108; // l
      view[3] = 108; // l
      view[4] = 111; // o

      const result = Utils.arrayBufferToString(buffer);

      expect(result).toBe("Hello");
    });
  });

  describe("Theme Management", () => {
    test("toggleTheme should toggle between light and dark themes", () => {
      document.body.classList.remove("dark");
      localStorage.setItem("theme", "light");
      window.dispatchEvent = jest.fn();

      const isDark = Utils.toggleTheme();

      expect(isDark).toBe(true);
      expect(document.body.classList.contains("dark")).toBe(true);
      expect(localStorage.getItem("theme")).toBe("dark");
      expect(window.dispatchEvent).toHaveBeenCalled();

      const isLight = Utils.toggleTheme();

      expect(isLight).toBe(false);
      expect(document.body.classList.contains("dark")).toBe(false);
      expect(localStorage.getItem("theme")).toBe("light");
    });
  });
});

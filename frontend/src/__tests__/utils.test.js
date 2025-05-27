import { Utils } from '../utils';
import { Base64 } from 'js-base64';

const localStorageMock = (() => {
  let store = {};
  return {
    getItem: jest.fn(key => store[key] || null),
    setItem: jest.fn((key, value) => {
      store[key] = value.toString();
    }),
    removeItem: jest.fn(key => {
      delete store[key];
    }),
    clear: jest.fn(() => {
      store = {};
    }),
    key: jest.fn(index => Object.keys(store)[index] || null),
    get length() {
      return Object.keys(store).length;
    }
  };
})();

jest.mock('js-base64', () => ({
  Base64: {
    encode: jest.fn(str => btoa(str)),
    decode: jest.fn(str => atob(str)),
    atob: jest.fn(str => atob(str)),
    btoa: jest.fn(str => btoa(str))
  }
}));

jest.mock('axios', () => ({
  get: jest.fn(() => Promise.resolve({ data: {} })),
  post: jest.fn(() => Promise.resolve({ data: {} }))
}));

describe('Utils', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'localStorage', { value: localStorageMock });
    Object.defineProperty(window, 'sessionStorage', { 
      value: { ...localStorageMock },
      writable: true
    });
    
    localStorageMock.clear();
    jest.clearAllMocks();
  });

  describe('Session Management', () => {
    test('addSession should add a new session', () => {
      const subdomain = 'test123';
      const token = 'test-token';
      
      Utils.sessions = [];
      Utils.addSession(subdomain, token);
      
      expect(Utils.sessions.length).toBe(1);
      expect(Utils.sessions[0].subdomain).toBe(subdomain);
      expect(Utils.sessions[0].token).toBe(token);
      expect(localStorageMock.setItem).toHaveBeenCalled();
    });

    test('addSession should throw error when max sessions reached', () => {
      Utils.sessions = Array(Utils.MAX_SESSIONS).fill().map((_, i) => ({
        subdomain: `test${i}`,
        token: `token${i}`,
        createdAt: new Date().toISOString(),
        unseenRequests: 0
      }));
      
      expect(() => Utils.addSession('newtest', 'newtoken')).toThrow('Maximum number of sessions reached');
    });

    test('removeSession should remove a session', () => {
      Utils.sessions = [
        { subdomain: 'test1', token: 'token1', createdAt: new Date().toISOString(), unseenRequests: 0 },
        { subdomain: 'test2', token: 'token2', createdAt: new Date().toISOString(), unseenRequests: 0 }
      ];
      Utils.selectedSessionIndex = 1;
      
      localStorageMock.key.mockImplementation(index => {
        const keys = [
          'requests_test2', 
          'visited_test2', 
          'lastRequest_test2', 
          'dns_test2', 
          'files_test2'
        ];
        return keys[index] || null;
      });
      
      const result = Utils.removeSession(1);
      
      expect(result).toBe(true);
      expect(Utils.sessions.length).toBe(1);
      expect(Utils.sessions[0].subdomain).toBe('test1');
      expect(Utils.selectedSessionIndex).toBe(0);
      expect(localStorageMock.removeItem).toHaveBeenCalled();
    });

    test('getSessionToken should return token for active session', () => {
      const mockSession = { subdomain: 'test1', token: 'token1' };
      jest.spyOn(Utils, 'getActiveSession').mockReturnValue(mockSession);
      
      const result = Utils.getSessionToken();
      
      expect(result).toBe('token1');
    });
  });

  describe('Encoding and Decoding', () => {
    test('base64Encode should encode strings correctly', () => {
      const testString = 'Hello World';
      const result = Utils.base64Encode(testString);
      
      expect(Base64.encode).toHaveBeenCalledWith(testString);
      expect(result).toBe(btoa(testString));
    });

    test('base64Decode should decode strings correctly', () => {
      const encoded = btoa('Hello World');
      const result = Utils.base64Decode(encoded);
      
      expect(Base64.atob).toHaveBeenCalledWith(encoded);
      expect(result).toBe('Hello World');
    });

    test('arrayBufferToString should convert ArrayBuffer to string', () => {
      const buffer = new ArrayBuffer(5);
      const view = new Uint8Array(buffer);
      view[0] = 72;  // H
      view[1] = 101; // e
      view[2] = 108; // l
      view[3] = 108; // l
      view[4] = 111; // o
      
      const result = Utils.arrayBufferToString(buffer);
      expect(result).toBe('Hello');
    });
  });

  describe('Theme Management', () => {
    test('toggleTheme should toggle between light and dark themes', () => {
      document.body.classList.remove('dark');
      localStorage.setItem('theme', 'light');
      
      window.dispatchEvent = jest.fn();
      
      const isDark = Utils.toggleTheme();
      
      expect(isDark).toBe(true);
      expect(document.body.classList.contains('dark')).toBe(true);
      expect(localStorage.getItem('theme')).toBe('dark');
      expect(window.dispatchEvent).toHaveBeenCalled();
      
      const isLight = Utils.toggleTheme();
      
      expect(isLight).toBe(false);
      expect(document.body.classList.contains('dark')).toBe(false);
      expect(localStorage.getItem('theme')).toBe('light');
    });
  });
});

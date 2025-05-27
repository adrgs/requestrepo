/* eslint-disable */
import "@testing-library/jest-dom";

global.getTestID = (id) => `test-id-${id}`;

jest.mock("../utils", () => {
  const originalModule = jest.requireActual("../utils");
  return {
    ...originalModule,
    getTestID: jest.fn((id) => `test-id-${id}`),
  };
});

Object.defineProperty(window, "matchMedia", {
  writable: true,
  value: jest.fn().mockImplementation((query) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: jest.fn(),
    removeListener: jest.fn(),
    addEventListener: jest.fn(),
    removeEventListener: jest.fn(),
    dispatchEvent: jest.fn(),
  })),
});

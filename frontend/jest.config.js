module.exports = {
  testEnvironment: "jsdom",
  transform: {
    "^.+\\.(js|jsx|ts|tsx)$": "babel-jest",
  },
  moduleNameMapper: {
    "\\.(css|less|scss|sass)$": "identity-obj-proxy",
    "\\.(jpg|jpeg|png|gif|webp|svg)$": "<rootDir>/__mocks__/fileMock.js",
    "^../utils$": "<rootDir>/src/__mocks__/utils.js",
  },
  setupFilesAfterEnv: ["<rootDir>/src/setupTests.js"],
  testMatch: [
    "**/__tests__/**/*.{js,jsx,ts,tsx}",
    "**/?(*.)+(spec|test).{js,jsx,ts,tsx}",
  ],
  collectCoverage: true,
  collectCoverageFrom: [
    "src/**/*.{js,jsx,ts,tsx}",
    "!src/**/*.d.ts",
    "!src/index.{jsx,tsx}",
    "!src/vite-env.d.ts",
    "!src/__mocks__/**",
  ],
  coverageReporters: ["text", "lcov"],
  testPathIgnorePatterns: ["/node_modules/"],
};

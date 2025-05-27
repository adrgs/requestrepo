import React from "react";
import { render, screen, act } from "@testing-library/react";
import { DnsSettingsPage } from "../../components/dns-settings-page";
import "@testing-library/jest-dom";

const mockGetSessionToken = jest.fn(() => "test-token");
const mockUpdateDNSRecords = jest.fn(() =>
  Promise.resolve({ msg: "Updated DNS records" }),
);
const mockGetDNSRecords = jest.fn(() =>
  Promise.resolve([{ domain: "", type: 0, value: "1.2.3.4" }]),
);
const mockIsDarkTheme = jest.fn(() => false);

jest.mock("../../utils", () => ({
  Utils: {
    toastOptions: {},
    getSessionToken: mockGetSessionToken,
    updateDNSRecords: mockUpdateDNSRecords,
    getDNSRecords: mockGetDNSRecords,
    domain: "example.com",
    isDarkTheme: mockIsDarkTheme,
  },
}));

const flushPromises = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("DnsSettingsPage Basic Tests", () => {
  const mockProps = {
    dnsRecords: [{ domain: "test", type: 0, value: "1.2.3.4" }],
    user: { subdomain: "test123" },
    toast: {
      error: jest.fn(),
      success: jest.fn(),
    },
    activeSession: { subdomain: "test123", token: "test-token" },
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test("renders DNS settings page with title", async () => {
    await act(async () => {
      render(<DnsSettingsPage {...mockProps} />);
      await flushPromises();
    });

    expect(screen.getByText("DNS Records")).toBeInTheDocument();
    expect(
      screen.getByText(/You can use % to select a random IP/i),
    ).toBeInTheDocument();

    expect(
      screen.getByRole("button", { name: /add record/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /save changes/i }),
    ).toBeInTheDocument();
  });
});

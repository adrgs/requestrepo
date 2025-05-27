import React from "react";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { DnsSettingsPage } from "../../components/dns-settings-page";
import "@testing-library/jest-dom";

jest.mock("../../components/record-input", () => ({
  RecordInput: ({ index }) => <div data-testid={`record-input-${index}`} />,
}));

jest.mock("primereact/button", () => ({
  Button: ({ label, onClick }) => (
    <button onClick={onClick} aria-label={label}>
      {label}
    </button>
  ),
}));

const mockUpdateDNSRecords = jest
  .fn()
  .mockResolvedValue({ msg: "Updated DNS records" });
const mockGetDNSRecords = jest
  .fn()
  .mockResolvedValue([{ domain: "", type: 0, value: "1.2.3.4" }]);
const mockGetSessionToken = jest.fn(() => "test-token");
const mockIsDarkTheme = jest.fn(() => false);
const mockRemoveSession = jest.fn();

jest.mock("../../utils", () => ({
  Utils: {
    toastOptions: {},
    getSessionToken: mockGetSessionToken,
    updateDNSRecords: mockUpdateDNSRecords,
    getDNSRecords: mockGetDNSRecords,
    domain: "example.com",
    isDarkTheme: mockIsDarkTheme,
    removeSession: mockRemoveSession,
  },
}));

const flushPromises = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("DnsSettingsPage Component", () => {
  const mockProps = {
    dnsRecords: [{ domain: "", type: 0, value: "1.2.3.4" }],
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
    let container;
    await act(async () => {
      const result = render(<DnsSettingsPage {...mockProps} />);
      container = result.container;
      await flushPromises();
    });

    expect(screen.getByText("DNS Records")).toBeInTheDocument();
  });

  test("adds a new record when add button is clicked", async () => {
    await act(async () => {
      render(<DnsSettingsPage {...mockProps} />);
      await flushPromises();
    });

    expect(screen.getByTestId("record-input-0")).toBeInTheDocument();

    await act(async () => {
      const addButton = screen.getByRole("button", { name: /add record/i });
      fireEvent.click(addButton);
      await flushPromises();
    });

    expect(screen.getAllByTestId(/record-input-/)).toHaveLength(2);
  });

  test("shows error when no active session", async () => {
    await act(async () => {
      render(<DnsSettingsPage {...mockProps} activeSession={null} />);
      await flushPromises();
    });

    await act(async () => {
      const addButton = screen.getByRole("button", { name: /add record/i });
      fireEvent.click(addButton);
      await flushPromises();
    });

    expect(mockProps.toast.error).toHaveBeenCalledWith(
      "No active session selected",
      expect.anything(),
    );
  });
});

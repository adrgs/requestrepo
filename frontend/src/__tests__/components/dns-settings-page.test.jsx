import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
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

jest.mock("../../utils", () => ({
  Utils: {
    toastOptions: {},
    getSessionToken: jest.fn(() => "test-token"),
    updateDNSRecords: mockUpdateDNSRecords,
    getDNSRecords: jest.fn(() =>
      Promise.resolve([{ domain: "", type: 0, value: "1.2.3.4" }]),
    ),
    domain: "example.com",
    isDarkTheme: jest.fn(() => false),
    removeSession: jest.fn(),
  },
}));

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

  test("renders DNS settings page with title", () => {
    render(<DnsSettingsPage {...mockProps} />);
    expect(screen.getByText("DNS Records")).toBeInTheDocument();
  });

  test("adds a new record when add button is clicked", async () => {
    render(<DnsSettingsPage {...mockProps} />);

    expect(screen.getByTestId("record-input-0")).toBeInTheDocument();

    const addButton = screen.getByRole("button", { name: /add record/i });
    fireEvent.click(addButton);

    await waitFor(() => {
      expect(screen.getAllByTestId(/record-input-/)).toHaveLength(2);
    });
  });

  test("shows error when no active session", () => {
    render(<DnsSettingsPage {...mockProps} activeSession={null} />);

    const addButton = screen.getByRole("button", { name: /add record/i });
    fireEvent.click(addButton);

    expect(mockProps.toast.error).toHaveBeenCalledWith(
      "No active session selected",
      expect.anything(),
    );
  });
});

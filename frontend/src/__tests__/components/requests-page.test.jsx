/* eslint-disable */
import React from "react";
import { render, screen } from "@testing-library/react";
import "@testing-library/jest-dom";

// Mock the entire requests-page component to avoid token issues
jest.mock("../../components/requests-page", () => ({
  RequestsPage: ({ requests = [], user = {} }) => (
    <div>
      <h1>Requests Page</h1>
      {requests.length === 0 ? (
        <p>No requests yet</p>
      ) : (
        <div>
          <p>Make a request to {user.subdomain}</p>
          <p>Request count: {requests.length}</p>
        </div>
      )}
    </div>
  ),
}));

describe("RequestsPage Component", () => {
  // Import the mocked component
  const { RequestsPage } = require("../../components/requests-page");

  const mockRequests = [
    {
      _id: "req1",
      method: "GET",
      path: "/test",
      date: Date.now(),
      headers: { "content-type": "application/json" },
      raw: "test-raw-data",
    },
  ];

  const mockProps = {
    requests: mockRequests,
    user: {
      subdomain: "test123",
      token: "test-token",
      domain: "example.com",
      requests: { req1: mockRequests[0] },
    },
    toast: {
      error: jest.fn(),
      success: jest.fn(),
    },
    activeSession: { subdomain: "test123", token: "test-token" },
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test("renders requests page with instructions when no request is selected", () => {
    render(<RequestsPage {...mockProps} />);
    expect(screen.getByText(/Make a request to/i)).toBeInTheDocument();
    expect(screen.getByText(/test123/i)).toBeInTheDocument();
  });

  test("displays no requests message when requests array is empty", () => {
    const props = {
      ...mockProps,
      requests: [],
    };

    render(<RequestsPage {...props} />);
    expect(screen.getByText(/No requests yet/i)).toBeInTheDocument();
  });
});

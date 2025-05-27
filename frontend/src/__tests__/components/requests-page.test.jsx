import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { RequestsPage } from '../../components/requests-page';
import '@testing-library/jest-dom';

jest.mock('../../utils', () => ({
  Utils: {
    toastOptions: {},
    getSessionToken: jest.fn(() => 'test-token'),
    base64Decode: jest.fn(str => str),
    isDarkTheme: jest.fn(() => false),
    getTestID: jest.fn(id => `test-id-${id}`)
  }
}));

describe('RequestsPage Component', () => {
  const mockRequests = [
    {
      _id: 'req1',
      method: 'GET',
      path: '/test',
      date: Date.now(),
      headers: { 'content-type': 'application/json' },
      raw: 'test-raw-data'
    }
  ];
  
  const mockProps = {
    requests: mockRequests,
    user: { subdomain: 'test123', token: 'test-token' },
    toast: {
      error: jest.fn(),
      success: jest.fn()
    },
    onDeleteRequest: jest.fn(),
    onDeleteAll: jest.fn(),
    selectedRequest: null,
    setSelectedRequest: jest.fn()
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test('renders requests page with instructions when no request is selected', () => {
    render(<RequestsPage {...mockProps} />);
    
    expect(screen.getByText(/Make a request to/i)).toBeInTheDocument();
    expect(screen.getByText(/test123/i)).toBeInTheDocument();
  });

  test('renders request details when a request is selected', () => {
    const props = {
      ...mockProps,
      selectedRequest: mockRequests[0]
    };
    
    render(<RequestsPage {...props} />);
    
    expect(screen.getByText(/GET/i)).toBeInTheDocument();
    expect(screen.getByText(/\/test/i)).toBeInTheDocument();
  });

  test('handles delete request', async () => {
    const props = {
      ...mockProps,
      selectedRequest: mockRequests[0]
    };
    
    render(<RequestsPage {...props} />);
    
    const deleteButton = screen.getByRole('button', { name: /delete/i });
    fireEvent.click(deleteButton);
    
    await waitFor(() => {
      expect(mockProps.onDeleteRequest).toHaveBeenCalledWith('req1');
    });
  });

  test('handles delete all requests', async () => {
    render(<RequestsPage {...mockProps} />);
    
    const deleteAllButton = screen.getByRole('button', { name: /delete all/i });
    fireEvent.click(deleteAllButton);
    
    await waitFor(() => {
      expect(mockProps.onDeleteAll).toHaveBeenCalled();
    });
  });

  test('displays no requests message when requests array is empty', () => {
    const props = {
      ...mockProps,
      requests: []
    };
    
    render(<RequestsPage {...props} />);
    
    expect(screen.getByText(/No requests yet/i)).toBeInTheDocument();
  });
});

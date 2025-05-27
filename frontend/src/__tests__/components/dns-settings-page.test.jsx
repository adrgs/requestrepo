import React from 'react';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { DnsSettingsPage } from '../../components/dns-settings-page';
import { Utils } from '../../utils';
import '@testing-library/jest-dom';

jest.mock('../../utils', () => ({
  Utils: {
    toastOptions: {},
    getSessionToken: jest.fn(() => 'test-token'),
    updateDNSRecords: jest.fn(() => Promise.resolve({ msg: 'Updated DNS records' })),
    getDNSRecords: jest.fn(() => Promise.resolve([])),
    domain: 'example.com',
    isDarkTheme: jest.fn(() => false)
  }
}));

describe('DnsSettingsPage Component', () => {
  const mockProps = {
    dnsRecords: [],
    user: { subdomain: 'test123' },
    toast: {
      error: jest.fn(),
      success: jest.fn()
    },
    activeSession: { subdomain: 'test123', token: 'test-token' }
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test('renders DNS settings page with title', () => {
    render(<DnsSettingsPage {...mockProps} />);
    
    expect(screen.getByText('DNS Records')).toBeInTheDocument();
    expect(screen.getByText(/You can use % to select a random IP/i)).toBeInTheDocument();
  });

  test('renders DNS records from props', () => {
    const props = {
      ...mockProps,
      dnsRecords: [
        { domain: 'test', type: 0, value: '1.2.3.4' },
        { domain: 'www', type: 2, value: 'example.com' }
      ]
    };
    
    render(<DnsSettingsPage {...props} />);
    
    const domainInputs = screen.getAllByPlaceholderText('Domain');
    expect(domainInputs.length).toBe(2);
    
    const valueInputs = screen.getAllByPlaceholderText('Value');
    expect(valueInputs.length).toBe(2);
  });

  test('adds a new record when add button is clicked', () => {
    render(<DnsSettingsPage {...mockProps} />);
    
    const addButton = screen.getByRole('button', { name: /add record/i });
    fireEvent.click(addButton);
    
    const domainInputs = screen.getAllByPlaceholderText('Domain');
    expect(domainInputs.length).toBe(1);
  });

  test('saves changes when save button is clicked', async () => {
    render(<DnsSettingsPage {...mockProps} />);
    
    const addButton = screen.getByRole('button', { name: /add record/i });
    fireEvent.click(addButton);
    
    const domainInput = screen.getByPlaceholderText('Domain');
    const valueInput = screen.getByPlaceholderText('Value');
    
    fireEvent.change(domainInput, { target: { value: 'test' } });
    fireEvent.change(valueInput, { target: { value: '1.2.3.4' } });
    
    const saveButton = screen.getByRole('button', { name: /save changes/i });
    fireEvent.click(saveButton);
    
    await waitFor(() => {
      expect(Utils.updateDNSRecords).toHaveBeenCalled();
      expect(mockProps.toast.success).toHaveBeenCalled();
    });
  });

  test('handles error when no active session', () => {
    const props = {
      ...mockProps,
      activeSession: null
    };
    
    render(<DnsSettingsPage {...props} />);
    
    const addButton = screen.getByRole('button', { name: /add record/i });
    fireEvent.click(addButton);
    
    expect(mockProps.toast.error).toHaveBeenCalledWith(
      'No active session selected',
      Utils.toastOptions
    );
  });
});

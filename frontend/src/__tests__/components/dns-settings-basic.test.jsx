import React from 'react';
import { render, screen } from '@testing-library/react';
import { DnsSettingsPage } from '../../components/dns-settings-page';
import '@testing-library/jest-dom';

jest.mock('../../utils', () => ({
  Utils: {
    toastOptions: {},
    getSessionToken: jest.fn(() => 'test-token'),
    updateDNSRecords: jest.fn(() => Promise.resolve({ msg: 'Updated DNS records' })),
    getDNSRecords: jest.fn(() => Promise.resolve([{ domain: '', type: 0, value: '1.2.3.4' }])),
    domain: 'example.com',
    isDarkTheme: jest.fn(() => false)
  }
}));

describe('DnsSettingsPage Basic Tests', () => {
  const mockProps = {
    dnsRecords: [{ domain: 'test', type: 0, value: '1.2.3.4' }],
    user: { subdomain: 'test123' },
    toast: {
      error: jest.fn(),
      success: jest.fn()
    },
    activeSession: { subdomain: 'test123', token: 'test-token' }
  };

  test('renders DNS settings page with title', () => {
    render(<DnsSettingsPage {...mockProps} />);
    
    expect(screen.getByText('DNS Records')).toBeInTheDocument();
    expect(screen.getByText(/You can use % to select a random IP/i)).toBeInTheDocument();
    
    expect(screen.getByRole('button', { name: /add record/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /save changes/i })).toBeInTheDocument();
  });
});

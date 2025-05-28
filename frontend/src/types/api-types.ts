export interface DNSRecord {
  domain: string;
  type: string | number;
  value: string;
  ttl: number;
  id?: string;
  subdomain?: string;
}

export interface DNSRecordResponse {
  success: boolean;
  message?: string;
  msg?: string;
  error?: string;
}

export interface DNSRecordRequest {
  records: DNSRecord[];
}

import { getDnsDomain } from "@/lib/config";
import type { AbuseCategory } from "@/types";

export const ABUSE_CATEGORIES: { key: AbuseCategory; label: string }[] = [
  { key: "malware", label: "Malware or botnet" },
  { key: "phishing", label: "Phishing" },
  { key: "csam", label: "Child sexual abuse material" },
  { key: "spam", label: "Spam" },
  { key: "other", label: "Other" },
];

export function categoryLabel(category: AbuseCategory): string {
  return (
    ABUSE_CATEGORIES.find((option) => option.key === category)?.label ??
    category
  );
}

/**
 * Reads the subdomain label out of what someone pasted: a label, a hostname or a URL.
 * This mirrors the server only well enough to preview and validate the form; the server
 * is what decides.
 */
export function extractSubdomain(value: string): string | null {
  let input = value.trim().toLowerCase();
  if (!input) return null;

  if (input.includes("://")) {
    try {
      input = new URL(input).hostname;
    } catch {
      return null;
    }
  }
  // "host.example.com:8080" without a scheme
  const port = input.lastIndexOf(":");
  if (port !== -1 && /^\d+$/.test(input.slice(port + 1))) {
    input = input.slice(0, port);
  }
  input = input.replace(/\.+$/, "");

  const suffix = `.${getDnsDomain().toLowerCase()}`;
  const label = input.endsWith(suffix) ? input.slice(0, -suffix.length) : input;
  return label && !label.includes(".") ? label : null;
}

/** Starts a download of `data` as a JSON file. */
export function downloadJson(filename: string, data: unknown): void {
  const blob = new Blob([JSON.stringify(data, null, 2)], {
    type: "application/json",
  });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

/** Starts a download of rows as a CSV file. */
export function downloadCsv(
  filename: string,
  header: string[],
  rows: (string | number | undefined)[][],
): void {
  const escape = (value: string | number | undefined) => {
    const text = String(value ?? "");
    return /[",\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text;
  };
  const csv = [header, ...rows]
    .map((row) => row.map(escape).join(","))
    .join("\n");
  const url = URL.createObjectURL(new Blob([csv + "\n"], { type: "text/csv" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

// The abuse admin token is kept for this tab only, and sent as a Bearer header. Storage can
// be unavailable (private windows, blocked site data), so every access is guarded.
const ADMIN_TOKEN_KEY = "abuse_admin_token";

export function loadAdminToken(): string | null {
  try {
    return sessionStorage.getItem(ADMIN_TOKEN_KEY);
  } catch {
    return null;
  }
}

export function saveAdminToken(token: string | null): void {
  try {
    if (token) sessionStorage.setItem(ADMIN_TOKEN_KEY, token);
    else sessionStorage.removeItem(ADMIN_TOKEN_KEY);
  } catch {
    // Kept in memory only
  }
}

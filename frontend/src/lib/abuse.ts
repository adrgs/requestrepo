import { getDnsDomain } from "@/lib/config";
import type { AbuseCategory } from "@/types";

export const ABUSE_CATEGORIES: { key: AbuseCategory; label: string }[] = [
  { key: "malware", label: "Malware or botnet" },
  { key: "phishing", label: "Phishing" },
  { key: "csam", label: "Child sexual abuse material" },
  { key: "spam", label: "Spam" },
  { key: "copyright", label: "Copyright infringement" },
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

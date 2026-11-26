import { getDnsDomain } from "@/lib/config";

/**
 * Reads the subdomain label out of what someone pasted: a label, a hostname under it or a
 * URL. This mirrors the server only well enough to validate the form; the server decides.
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
  const rest = input.endsWith(suffix) ? input.slice(0, -suffix.length) : input;
  // A name under the subdomain belongs to it too
  const label = rest.split(".").pop() ?? "";
  return label || null;
}

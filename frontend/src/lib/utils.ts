import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString();
}

export function formatRelativeTime(timestamp: number): string {
  const now = Date.now();
  const diff = now - timestamp * 1000;

  if (diff < 60000) return "just now";
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
  return formatDate(timestamp);
}

export function truncate(str: string, maxLength: number): string {
  if (str.length <= maxLength) return str;
  return str.slice(0, maxLength - 3) + "...";
}

export function copyToClipboard(text: string): Promise<void> {
  return navigator.clipboard.writeText(text);
}

export function getMethodColor(method: string | undefined | null): string {
  if (!method) return "default";
  const colors: Record<string, string> = {
    GET: "success",
    POST: "primary",
    PUT: "warning",
    PATCH: "warning",
    DELETE: "danger",
    HEAD: "secondary",
    OPTIONS: "secondary",
  };
  return colors[method.toUpperCase()] ?? "default";
}

export function getDnsTypeColor(type: string | undefined | null): string {
  if (!type) return "default";
  const colors: Record<string, string> = {
    A: "primary",
    AAAA: "secondary",
    CNAME: "warning",
    TXT: "success",
    MX: "danger",
  };
  return colors[type.toUpperCase()] ?? "default";
}

/**
 * Get the flag-icons CSS class for a country code.
 * Uses the flag-icons library bundled locally.
 * Example: "DE" -> "fi fi-de", "US" -> "fi fi-us"
 */
export function getFlagClass(countryCode: string | null | undefined): string {
  if (!countryCode || countryCode.length !== 2) return "fi fi-aq"; // Antarctica as fallback
  return `fi fi-${countryCode.toLowerCase()}`;
}

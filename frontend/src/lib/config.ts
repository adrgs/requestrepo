/**
 * Frontend configuration
 *
 * Uses runtime config injected by the backend (window.__CONFIG__.DOMAIN)
 * Falls back to window.location-based detection for development
 */

// Detect if we're running in development mode
const isDev = import.meta.env.DEV;

/**
 * Get the base domain from runtime config or fallback to detection
 * Priority:
 * 1. Backend-injected window.__CONFIG__.DOMAIN (most reliable)
 * 2. VITE_DOMAIN env var (for development)
 * 3. Window location-based detection (fallback)
 */
function getRuntimeDomain(): string {
  if (typeof window !== "undefined") {
    // Use backend-injected domain if available (most reliable)
    if (window.__CONFIG__?.DOMAIN) {
      return window.__CONFIG__.DOMAIN;
    }

    // Fallback: try to detect from hostname
    const hostname = window.location.hostname;
    const parts = hostname.split(".");

    // Handle localhost or single-part hostnames
    if (parts.length <= 2) {
      return hostname;
    }

    // Use VITE_DOMAIN to determine base domain part count
    const viteDomain = import.meta.env.VITE_DOMAIN || "";
    const viteParts = viteDomain.split(".").filter(Boolean);
    const baseDomainParts = viteParts.length >= 2 ? viteParts.length : 2;

    // Return the last N parts as base domain
    return parts.slice(-baseDomainParts).join(".");
  }

  // Fallback for build time / SSR
  return import.meta.env.VITE_DOMAIN || "localhost";
}

// Get domain at runtime
const domain = getRuntimeDomain();

// Check if this is production (requestrepo.com)
export const isProduction = domain === "requestrepo.com";

// Get the DNS domain (no port - DNS doesn't use ports)
export function getDnsDomain(): string {
  return getRuntimeDomain();
}

// Get the base domain for URLs (includes port only if non-standard)
export function getBaseDomain(): string {
  if (typeof window !== "undefined") {
    const baseDomain = getRuntimeDomain();

    // In dev mode, use VITE_HTTP_PORT (backend port), not window.location.port (Vite dev server)
    const port = isDev
      ? import.meta.env.VITE_HTTP_PORT || "21337"
      : window.location.port;

    // Include port if non-standard (not 80/443)
    if (port && port !== "80" && port !== "443") {
      return `${baseDomain}:${port}`;
    }
    return baseDomain;
  }

  // Fallback for build time
  const fallbackDomain = import.meta.env.VITE_DOMAIN || "localhost";
  const fallbackPort = import.meta.env.VITE_HTTP_PORT || "21337";

  if (fallbackPort !== "80" && fallbackPort !== "443") {
    return `${fallbackDomain}:${fallbackPort}`;
  }
  return fallbackDomain;
}

// Get the full URL for a subdomain (for copying/sharing)
export function getSubdomainUrl(subdomain: string): string {
  const baseDomain = getBaseDomain();
  const protocol =
    typeof window !== "undefined" ? window.location.protocol : "http:";
  return `${protocol}//${subdomain}.${baseDomain}`;
}

// Export config object for easy access
export const config = {
  isProduction,
  isDev,
  domain,
  getBaseDomain,
  getDnsDomain,
  getSubdomainUrl,
};

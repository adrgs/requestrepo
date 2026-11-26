/**
 * Reads a session token without verifying it.
 *
 * The signature is checked by the server on every request. This only answers "which
 * session is this response about", which the dashboard needs when the server refuses one.
 */
export function getSubdomainFromToken(token: string): string | null {
  const payload = token.split(".")[1];
  if (!payload) return null;
  try {
    // JWTs use base64url; atob wants the standard alphabet
    const base64 = payload.replace(/-/g, "+").replace(/_/g, "/");
    const claims: unknown = JSON.parse(atob(base64));
    if (claims && typeof claims === "object" && "subdomain" in claims) {
      const subdomain = claims.subdomain;
      return typeof subdomain === "string" && subdomain ? subdomain : null;
    }
    return null;
  } catch {
    return null;
  }
}

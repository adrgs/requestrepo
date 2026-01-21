/**
 * Encode a string to base64
 * Processes bytes in chunks to avoid call stack overflow with large data
 */
export function encodeBase64(str: string): string {
  const bytes = new TextEncoder().encode(str);
  // Process in chunks to avoid call stack overflow with spread operator
  const CHUNK_SIZE = 0x8000; // 32KB chunks
  let binary = "";
  for (let i = 0; i < bytes.length; i += CHUNK_SIZE) {
    const chunk = bytes.subarray(i, i + CHUNK_SIZE);
    binary += String.fromCharCode.apply(null, chunk as unknown as number[]);
  }
  return btoa(binary);
}

/**
 * Decode a base64 string
 */
export function decodeBase64(base64: string): string {
  try {
    const binary = atob(base64);
    const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
    return new TextDecoder().decode(bytes);
  } catch {
    return "";
  }
}

/**
 * Check if a string is valid base64
 */
export function isValidBase64(str: string): boolean {
  try {
    atob(str);
    return true;
  } catch {
    return false;
  }
}

/**
 * Decode base64 and check if it's printable text
 */
export function decodeBase64Safe(base64: string): {
  text: string;
  isPrintable: boolean;
} {
  const decoded = decodeBase64(base64);
  const isPrintable = /^[\x20-\x7E\t\n\r]*$/.test(decoded);
  return { text: decoded, isPrintable };
}

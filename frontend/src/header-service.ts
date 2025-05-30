export class HeaderService {
  static parseHeaders(
    headersString: string,
  ): Array<{ key: string; value: string }> {
    if (!headersString || headersString.trim() === "") {
      return [];
    }

    return headersString
      .split("\n")
      .filter((line) => line.trim() !== "")
      .map((line) => {
        const colonIndex = line.indexOf(":");
        if (colonIndex === -1) {
          return { key: line.trim(), value: "" };
        }
        const key = line.substring(0, colonIndex).trim();
        const value = line.substring(colonIndex + 1).trim();
        return { key, value };
      });
  }

  static stringifyHeaders(
    headers: Array<{ key: string; value: string }>,
  ): string {
    if (!headers || headers.length === 0) {
      return "";
    }

    return headers
      .filter((header) => header.key.trim() !== "")
      .map((header) => `${header.key}: ${header.value}`)
      .join("\n");
  }

  static getContentType(
    headers: Array<{ key: string; value: string }>,
  ): string {
    const contentTypeHeader = headers.find(
      (h) => h.key.toLowerCase() === "content-type",
    );
    return contentTypeHeader ? contentTypeHeader.value : "";
  }

  static updateContentType(
    headers: Array<{ key: string; value: string }>,
    contentType: string,
  ): Array<{ key: string; value: string }> {
    const newHeaders = [...headers];
    const contentTypeIndex = newHeaders.findIndex(
      (h) => h.key.toLowerCase() === "content-type",
    );

    if (contentTypeIndex >= 0) {
      newHeaders[contentTypeIndex] = {
        key: "Content-Type",
        value: contentType,
      };
    } else {
      newHeaders.push({ key: "Content-Type", value: contentType });
    }

    return newHeaders;
  }
}

import type { FileTree } from "@/types";

export interface TreeNode {
  name: string;
  path: string; // Full path (key in FileTree for files, empty for folders)
  type: "file" | "folder";
  children: TreeNode[];
}

/**
 * Build a tree structure from a flat FileTree
 * Paths like "folder/subfolder/file.html" become nested nodes
 */
export function buildFileTree(files: FileTree): TreeNode[] {
  const root: TreeNode[] = [];

  // Get all file paths and sort them
  const paths = Object.keys(files).sort();

  for (const path of paths) {
    const parts = path.split("/");
    let currentLevel = root;

    for (let i = 0; i < parts.length; i++) {
      const part = parts[i];
      const isLast = i === parts.length - 1;

      // Find existing node at this level
      let existing = currentLevel.find((n) => n.name === part);

      if (!existing) {
        const newNode: TreeNode = {
          name: part,
          path: isLast ? path : "", // Only files have paths
          type: isLast ? "file" : "folder",
          children: [],
        };
        currentLevel.push(newNode);
        existing = newNode;
      } else if (isLast) {
        // This shouldn't happen with valid file paths, but handle gracefully
        existing.type = "file";
        existing.path = path;
      }

      currentLevel = existing.children;
    }
  }

  return sortTreeNodes(root);
}

/**
 * Sort tree nodes: folders first (alphabetically), then files (alphabetically)
 */
export function sortTreeNodes(nodes: TreeNode[]): TreeNode[] {
  const sorted = [...nodes].sort((a, b) => {
    // Folders before files
    if (a.type === "folder" && b.type === "file") return -1;
    if (a.type === "file" && b.type === "folder") return 1;
    // Alphabetically within same type
    return a.name.localeCompare(b.name);
  });

  // Recursively sort children
  for (const node of sorted) {
    if (node.children.length > 0) {
      node.children = sortTreeNodes(node.children);
    }
  }

  return sorted;
}

/**
 * Get the parent path of a file or folder path
 * Returns null for root-level items
 */
export function getParentPath(path: string): string | null {
  const lastSlash = path.lastIndexOf("/");
  if (lastSlash === -1) return null;
  return path.substring(0, lastSlash);
}

/**
 * Get the file/folder name from a path
 */
export function getFileName(path: string): string {
  const lastSlash = path.lastIndexOf("/");
  if (lastSlash === -1) return path;
  return path.substring(lastSlash + 1);
}

/**
 * Check if a path is inside a folder path
 */
export function isPathInFolder(path: string, folderPath: string): boolean {
  if (!folderPath) return path.indexOf("/") === -1; // Root level
  return path.startsWith(folderPath + "/");
}

/**
 * Validate a file name (no slashes, not empty, no leading/trailing spaces)
 */
export function isValidFileName(name: string): boolean {
  if (!name || name.trim() !== name) return false;
  if (name.includes("/") || name.includes("\\")) return false;
  if (name === "." || name === "..") return false;
  // Check for invalid characters (control chars are intentionally blocked)
  // eslint-disable-next-line no-control-regex
  if (/[<>:"|?*\x00-\x1f]/.test(name)) return false;
  return true;
}

/**
 * Validate a folder name (same rules as file name)
 */
export function isValidFolderName(name: string): boolean {
  return isValidFileName(name);
}

/**
 * Get all file paths that are inside a folder
 */
export function getFilesInFolder(
  files: FileTree,
  folderPath: string,
): string[] {
  return Object.keys(files).filter((path) => isPathInFolder(path, folderPath));
}

/**
 * Check if a folder is empty (no files inside)
 */
export function isFolderEmpty(files: FileTree, folderPath: string): boolean {
  return getFilesInFolder(files, folderPath).length === 0;
}

/**
 * Rename a file or folder and update all affected paths
 */
export function renameInTree(
  files: FileTree,
  oldPath: string,
  newPath: string,
): FileTree {
  const newFiles: FileTree = {};

  for (const [path, entry] of Object.entries(files)) {
    if (path === oldPath) {
      // This is the renamed item
      newFiles[newPath] = entry;
    } else if (path.startsWith(oldPath + "/")) {
      // This is inside a renamed folder
      const newFilePath = newPath + path.substring(oldPath.length);
      newFiles[newFilePath] = entry;
    } else {
      // Unchanged
      newFiles[path] = entry;
    }
  }

  return newFiles;
}

/**
 * Delete a file or folder and all its contents
 */
export function deleteFromTree(files: FileTree, path: string): FileTree {
  const newFiles: FileTree = {};

  for (const [filePath, entry] of Object.entries(files)) {
    // Keep if not the deleted item and not inside deleted folder
    if (filePath !== path && !filePath.startsWith(path + "/")) {
      newFiles[filePath] = entry;
    }
  }

  return newFiles;
}

/**
 * Get the Content-Type header value based on file extension
 */
export function getContentTypeFromExtension(filename: string): string {
  const ext = filename.split(".").pop()?.toLowerCase() || "";
  const mimeTypes: Record<string, string> = {
    // Text/Code
    html: "text/html; charset=utf-8",
    htm: "text/html; charset=utf-8",
    css: "text/css; charset=utf-8",
    js: "text/javascript; charset=utf-8",
    mjs: "text/javascript; charset=utf-8",
    json: "application/json; charset=utf-8",
    xml: "application/xml; charset=utf-8",
    txt: "text/plain; charset=utf-8",
    md: "text/markdown; charset=utf-8",
    csv: "text/csv; charset=utf-8",
    yaml: "text/yaml; charset=utf-8",
    yml: "text/yaml; charset=utf-8",
    // Images
    png: "image/png",
    jpg: "image/jpeg",
    jpeg: "image/jpeg",
    gif: "image/gif",
    svg: "image/svg+xml",
    ico: "image/x-icon",
    webp: "image/webp",
    // Fonts
    woff: "font/woff",
    woff2: "font/woff2",
    ttf: "font/ttf",
    otf: "font/otf",
    eot: "application/vnd.ms-fontobject",
    // Other
    pdf: "application/pdf",
    zip: "application/zip",
    wasm: "application/wasm",
  };
  return mimeTypes[ext] || "text/plain; charset=utf-8";
}

/**
 * Get unique folder paths from a FileTree
 */
export function getFolderPaths(files: FileTree): string[] {
  const folders = new Set<string>();

  for (const path of Object.keys(files)) {
    const parts = path.split("/");
    // Add all parent folders
    for (let i = 1; i < parts.length; i++) {
      folders.add(parts.slice(0, i).join("/"));
    }
  }

  return Array.from(folders).sort();
}

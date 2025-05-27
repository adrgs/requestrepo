import React, { useState, useRef, useEffect } from "react";
import { Tree, TreeSelectionParams } from "primereact/tree";
import { ContextMenu } from "primereact/contextmenu";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import { Utils } from "../utils";
import "./file-tree.scss";

interface FileTreeProps {
  files: Record<string, unknown>;
  selectedFile: string | null;
  onSelect: (path: string) => void;
  onUpdate: (files: Record<string, unknown>) => void;
  toast: {
    success: (message: string, options?: Record<string, unknown>) => void;
    error: (message: string, options?: Record<string, unknown>) => void;
  };
}

interface TreeNode {
  key: string;
  label: string;
  data: string;
  icon?: string;
  children?: TreeNode[];
  leaf?: boolean;
}

export const FileTree: React.FC<FileTreeProps> = ({
  files,
  selectedFile,
  onSelect,
  onUpdate,
  toast,
}) => {
  const [editingNode, setEditingNode] = useState<TreeNode | null>(null);
  const [editingText, setEditingText] = useState<string>("");
  const [editMode, setEditMode] = useState<string>(""); // "new", "rename"
  const [isDark, setIsDark] = useState<boolean>(Utils.isDarkTheme());
  const cm = useRef<ContextMenu | null>(null);

  useEffect(() => {
    const handleThemeChange = (): void => {
      setIsDark(Utils.isDarkTheme());
    };
    window.addEventListener("themeChange", handleThemeChange);
    return () => window.removeEventListener("themeChange", handleThemeChange);
  }, []);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent): void => {
      if (editMode && !(e.target as HTMLElement).closest(".p-contextmenu")) {
        setEditMode("");
        setEditingNode(null);
        setEditingText("");
        cm.current?.hide(e as unknown as React.SyntheticEvent);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [editMode]);

  const convertToTreeNodes = (
    data: Record<string, unknown>,
    parentPath = "",
  ): TreeNode[] => {
    return Object.entries(data).map(([key, value]) => {
      const currentPath = parentPath + key;
      const isDirectory = key.endsWith("/");

      if (isDirectory) {
        const label = key.slice(0, -1);
        return {
          key: currentPath,
          label,
          data: currentPath,
          icon: "pi pi-fw pi-folder" + (isDark ? "-open" : ""),
          children: convertToTreeNodes(value as Record<string, unknown>, currentPath),
        };
      } else {
        return {
          key: currentPath,
          label: key,
          data: currentPath,
          icon: "pi pi-fw pi-file",
          leaf: true,
        };
      }
    });
  };

  const handleNodeClick = (node: TreeNode): void => {
    if (!node.children) {
      onSelect(node.data);
    }
  };

  const handleDelete = (node: TreeNode): void => {
    const path = node.data;
    const newFiles = { ...files };
    let current = newFiles;
    const parts = path.split("/");

    for (let i = 0; i < parts.length - 1; i++) {
      if (parts[i] + "/" in current) {
        current = current[parts[i] + "/"] as Record<string, unknown>;
      }
    }

    const lastPart = parts[parts.length - 1];
    delete current[lastPart];

    onUpdate(newFiles);
    toast.success("File deleted successfully", Utils.toastOptions);
  };

  const handleRename = (node: TreeNode): void => {
    if (!editingText.trim()) {
      toast.error("Filename cannot be empty", Utils.toastOptions);
      return;
    }

    const path = node.data;
    const newFiles = { ...files };
    let current = newFiles;
    const parts = path.split("/");

    for (let i = 0; i < parts.length - 1; i++) {
      if (parts[i] + "/" in current) {
        current = current[parts[i] + "/"] as Record<string, unknown>;
      }
    }

    const lastPart = parts[parts.length - 1];
    const isDirectory = lastPart.endsWith("/");
    const newName = isDirectory ? editingText + "/" : editingText;

    if (newName in current) {
      toast.error(
        "A file with this name already exists",
        Utils.toastOptions,
      );
      return;
    }

    current[newName] = current[lastPart];
    delete current[lastPart];

    onUpdate(newFiles);
    setEditMode("");
    setEditingNode(null);
    setEditingText("");
    toast.success("File renamed successfully", Utils.toastOptions);
  };

  const getContextMenuItems = (node: TreeNode) => {
    const isDirectory = node.key.endsWith("/");
    const items = [
      {
        label: "Rename",
        icon: "pi pi-fw pi-pencil",
        command: () => {
          setEditingNode(node);
          setEditingText(node.label);
          setEditMode("rename");
        },
      },
      {
        label: "Delete",
        icon: "pi pi-fw pi-trash",
        command: () => handleDelete(node),
      },
    ];

    if (isDirectory) {
      items.unshift({
        label: "New File",
        icon: "pi pi-fw pi-file",
        command: () => {
          setEditingNode(node);
          setEditingText("");
          setEditMode("new");
        },
      });

      interface MenuItem {
        label: string;
        icon?: string;
        command?: () => void;
        items?: MenuItem[];
      }
      
      (items as MenuItem[]).push({
        label: "New File with Template",
        icon: "pi pi-fw pi-file-o",
        items: [
          {
            label: "HTML",
            command: () => {
              const newFiles = { ...files };
              let current = newFiles;
              const parts = node.data.split("/");

              for (let i = 0; i < parts.length; i++) {
                if (parts[i] + "/" in current) {
                  current = current[parts[i] + "/"] as Record<string, unknown>;
                }
              }

              let filename = "index.html";
              let counter = 1;
              while (filename in current) {
                filename = `index${counter}.html`;
                counter++;
              }

              current[filename] = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Document</title>
</head>
<body>
    
</body>
</html>`;

              onUpdate(newFiles);
              toast.success("HTML file created", Utils.toastOptions);
            },
          },
          {
            label: "CSS",
            command: () => {
              const newFiles = { ...files };
              let current = newFiles;
              const parts = node.data.split("/");

              for (let i = 0; i < parts.length; i++) {
                if (parts[i] + "/" in current) {
                  current = current[parts[i] + "/"] as Record<string, unknown>;
                }
              }

              let filename = "styles.css";
              let counter = 1;
              while (filename in current) {
                filename = `styles${counter}.css`;
                counter++;
              }

              current[filename] = `/* CSS Styles */
body {
    font-family: Arial, sans-serif;
    margin: 0;
    padding: 0;
}`;

              onUpdate(newFiles);
              toast.success("CSS file created", Utils.toastOptions);
            },
          },
          {
            label: "JavaScript",
            command: () => {
              const newFiles = { ...files };
              let current = newFiles;
              const parts = node.data.split("/");

              for (let i = 0; i < parts.length; i++) {
                if (parts[i] + "/" in current) {
                  current = current[parts[i] + "/"] as Record<string, unknown>;
                }
              }

              let filename = "script.js";
              let counter = 1;
              while (filename in current) {
                filename = `script${counter}.js`;
                counter++;
              }

              current[filename] = `// JavaScript
document.addEventListener('DOMContentLoaded', function() {
    console.log('Document ready!');
});`;

              onUpdate(newFiles);
              toast.success(
                "JavaScript file created",
                Utils.toastOptions,
              );
            },
          },
          {
            label: "JSON",
            command: () => {
              const newFiles = { ...files };
              let current = newFiles;
              const parts = node.data.split("/");

              for (let i = 0; i < parts.length; i++) {
                if (parts[i] + "/" in current) {
                  current = current[parts[i] + "/"] as Record<string, unknown>;
                }
              }

              let filename = "data.json";
              let counter = 1;
              while (filename in current) {
                filename = `data${counter}.json`;
                counter++;
              }

              current[filename] = `{
    "name": "Example",
    "version": "1.0.0",
    "description": "Example JSON file"
}`;

              onUpdate(newFiles);
              toast.success("JSON file created", Utils.toastOptions);
            },
          },
        ],
      });

      items.push({
        label: "Add Default Headers",
        icon: "pi pi-fw pi-list",
        command: () => {
          const newFiles = { ...files };
          let current = newFiles;
          const parts = node.data.split("/");

          for (let i = 0; i < parts.length; i++) {
            if (parts[i] + "/" in current) {
              current = current[parts[i] + "/"] as Record<string, unknown>;
            }
          }

          let filename = "headers.txt";
          let counter = 1;
          while (filename in current) {
            filename = `headers${counter}.txt`;
            counter++;
          }

          current[filename] = getDefaultHeaders();

          onUpdate(newFiles);
          toast.success("Headers file created", Utils.toastOptions);
        },
      });
    }

    return items;
  };

  const handleContextMenu = (event: { node: TreeNode; originalEvent: React.MouseEvent }): void => {
    if (event.node) {
      cm.current?.show(event.originalEvent);
      event.originalEvent.preventDefault();
    }
  };

  const getDefaultHeaders = (): string => {
    const contentTypes = {
      html: "Content-Type: text/html; charset=utf-8",
      css: "Content-Type: text/css; charset=utf-8",
      js: "Content-Type: application/javascript; charset=utf-8",
      json: "Content-Type: application/json; charset=utf-8",
      xml: "Content-Type: application/xml; charset=utf-8",
      pdf: "Content-Type: application/pdf",
      png: "Content-Type: image/png",
      jpg: "Content-Type: image/jpeg",
      gif: "Content-Type: image/gif",
      svg: "Content-Type: image/svg+xml",
      webp: "Content-Type: image/webp",
      mp4: "Content-Type: video/mp4",
      mp3: "Content-Type: audio/mpeg",
      woff: "Content-Type: font/woff",
      woff2: "Content-Type: font/woff2",
      ttf: "Content-Type: font/ttf",
      otf: "Content-Type: font/otf",
      eot: "Content-Type: application/vnd.ms-fontobject",
      zip: "Content-Type: application/zip",
      csv: "Content-Type: text/csv; charset=utf-8",
      txt: "Content-Type: text/plain; charset=utf-8",
    };

    return `HTTP/1.1 200 OK
Server: requestrepo
${contentTypes.html}
Cache-Control: no-cache, no-store, must-revalidate
Pragma: no-cache
Expires: 0
Access-Control-Allow-Origin: *
Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS
Access-Control-Allow-Headers: Content-Type, Authorization
Content-Length: 0

`;
  };

  const createNewItem = (): void => {
    if (!editingText.trim()) {
      toast.error("Filename cannot be empty", Utils.toastOptions);
      return;
    }

    const newFiles = { ...files };
    let current = newFiles;
    const path = editingNode?.data || "";
    const parts = path.split("/");

    for (let i = 0; i < parts.length; i++) {
      if (parts[i] + "/" in current) {
        current = current[parts[i] + "/"] as Record<string, unknown>;
      }
    }

    const lastPart = editingText;
    if (lastPart in current) {
      toast.error(
        "A file with this name already exists",
        Utils.toastOptions,
      );
      return;
    }

    current[lastPart] = "";

    onUpdate(newFiles);
    setEditMode("");
    setEditingNode(null);
    setEditingText("");
    toast.success("File created successfully", Utils.toastOptions);
  };

  const treeNodes = convertToTreeNodes(files);

  const getFileFromPath = (path: string): TreeNode | null => {
    if (!path) return null;

    const findNode = (nodes: TreeNode[]): TreeNode | null => {
      for (const node of nodes) {
        if (node.data === path) {
          return node;
        }
        if (node.children) {
          const found = findNode(node.children);
          if (found) return found;
        }
      }
      return null;
    };

    return findNode(treeNodes);
  };

  const selectedNode = selectedFile ? getFileFromPath(selectedFile) : null;
  const selectedKeys = selectedNode ? { [selectedNode.key]: true } : {};

  return (
    <div className="file-tree-container">
      <div className="file-tree-header">
        <h3>Files</h3>
      </div>
      <div className="file-tree-content">
        <Tree
          value={treeNodes}
          selectionMode="single"
          selectionKeys={selectedKeys}
          onSelectionChange={(e: TreeSelectionParams) => {
            if (e.value && typeof e.value === 'object') {
              const key = Object.keys(e.value)[0];
              const node = getFileFromPath(key);
              if (node) handleNodeClick(node);
            }
          }}
          onContextMenu={handleContextMenu}
          className={isDark ? "dark-tree" : ""}
        />
        <ContextMenu
          model={editingNode ? getContextMenuItems(editingNode) : []}
          ref={cm}
        />

        {editMode === "rename" && (
          <div className="p-dialog-mask">
            <div className="p-dialog">
              <div className="p-dialog-header">
                <span>Rename File</span>
              </div>
              <div className="p-dialog-content">
                <InputText
                  value={editingText}
                  onChange={(e) => setEditingText(e.target.value)}
                  autoFocus
                />
              </div>
              <div className="p-dialog-footer">
                <Button
                  label="Cancel"
                  icon="pi pi-times"
                  onClick={() => {
                    setEditMode("");
                    setEditingNode(null);
                    setEditingText("");
                  }}
                  className="p-button-text"
                />
                <Button
                  label="Save"
                  icon="pi pi-check"
                  onClick={() => handleRename(editingNode as TreeNode)}
                  autoFocus
                />
              </div>
            </div>
          </div>
        )}

        {editMode === "new" && (
          <div className="p-dialog-mask">
            <div className="p-dialog">
              <div className="p-dialog-header">
                <span>New File</span>
              </div>
              <div className="p-dialog-content">
                <InputText
                  value={editingText}
                  onChange={(e) => setEditingText(e.target.value)}
                  placeholder="Enter filename"
                  autoFocus
                />
              </div>
              <div className="p-dialog-footer">
                <Button
                  label="Cancel"
                  icon="pi pi-times"
                  onClick={() => {
                    setEditMode("");
                    setEditingNode(null);
                    setEditingText("");
                  }}
                  className="p-button-text"
                />
                <Button
                  label="Create"
                  icon="pi pi-check"
                  onClick={createNewItem}
                  autoFocus
                />
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

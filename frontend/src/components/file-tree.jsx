import React, { useState, useRef, useEffect } from "react";
import { Tree } from "primereact/tree";
import { ContextMenu } from "primereact/contextmenu";
import { InputText } from "primereact/inputtext";
import { Button } from "primereact/button";
import { Utils } from "../utils";
import "./file-tree.scss";
import PropTypes from "prop-types";

export const FileTree = ({
  files,
  selectedFile,
  onSelect,
  onUpdate,
  toast,
}) => {
  const [editingNode, setEditingNode] = useState(null);
  const [editingText, setEditingText] = useState("");
  const [editMode, setEditMode] = useState(""); // "new", "rename"
  const [isDark, setIsDark] = useState(Utils.isDarkTheme());
  const cm = useRef(null);

  useEffect(() => {
    const handleThemeChange = () => {
      setIsDark(Utils.isDarkTheme());
    };
    window.addEventListener("themeChange", handleThemeChange);
    return () => window.removeEventListener("themeChange", handleThemeChange);
  }, []);

  useEffect(() => {
    const handleClickOutside = (e) => {
      if (editMode && !e.target.closest(".p-contextmenu")) {
        setEditMode("");
        setEditingNode(null);
        setEditingText("");
        cm.current?.hide();
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [editMode]);

  const convertToTreeNodes = (data, parentPath = "") => {
    return Object.entries(data).map(([key, value]) => {
      const currentPath = parentPath + key;
      const isDirectory = key.endsWith("/");

      if (isDirectory) {
        const label = key.slice(0, -1);
        return {
          key: currentPath,
          label,
          data: currentPath,
          icon: "pi pi-folder",
          children: convertToTreeNodes(value, currentPath),
          type: "directory",
        };
      } else {
        return {
          key: currentPath,
          label: key,
          data: currentPath,
          icon: "pi pi-file",
          type: "file",
        };
      }
    });
  };

  const handleNodeClick = (node) => {
    if (!node) return;
    if (!node.endsWith("/")) {
      const file = getFileFromPath(node);
      if (file) {
        onSelect({
          path: node,
          ...file,
        });
      }
    }
  };

  const handleDelete = (path) => {
    const updatedFiles = { ...files };
    let current = updatedFiles;
    const parts = path.split("/").filter((p) => p);

    // Navigate to parent
    for (let i = 0; i < parts.length - 1; i++) {
      const part = parts[i] + "/";
      if (!(part in current)) return;
      current = current[part];
    }

    // Delete the item
    const lastPart = parts[parts.length - 1] + (path.endsWith("/") ? "/" : "");
    delete current[lastPart];

    onUpdate(updatedFiles);
  };

  const handleRename = (oldPath, newName) => {
    if (!newName) return;

    const updatedFiles = { ...files };
    let current = updatedFiles;
    const parts = oldPath.split("/").filter((p) => p);

    // Navigate to parent
    for (let i = 0; i < parts.length - 1; i++) {
      const part = parts[i] + "/";
      if (!(part in current)) return;
      current = current[part];
    }

    // Rename the item
    const lastPart =
      parts[parts.length - 1] + (oldPath.endsWith("/") ? "/" : "");
    const newPart = newName + (oldPath.endsWith("/") ? "/" : "");

    // If the name hasn't changed, do nothing
    if (lastPart === newPart) {
      setEditMode("");
      setEditingNode(null);
      setEditingText("");
      cm.current?.hide();
      return;
    }

    // If the new name already exists, don't overwrite it
    if (newPart in current) {
      toast.error("A file with that name already exists", Utils.toastOptions);
      return;
    }

    current[newPart] = current[lastPart];
    delete current[lastPart];

    onUpdate(updatedFiles);
  };

  const getContextMenuItems = (node) => {
    if (!node) {
      // Root level context menu
      if (editMode === "new-file" || editMode === "new-folder") {
        return [
          {
            template: () => (
              <div className="inline-edit" onClick={(e) => e.preventDefault()}>
                <InputText
                  value={editingText}
                  onChange={(e) => {
                    e.stopPropagation();
                    setEditingText(e.target.value);
                  }}
                  onKeyDown={(e) => {
                    e.stopPropagation();
                    if (e.key === "Enter") {
                      createNewItem(editMode === "new-folder");
                      setEditMode("");
                      setEditingNode(null);
                      setEditingText("");
                      cm.current?.hide();
                    }
                    if (e.key === "Escape") {
                      setEditMode("");
                      setEditingNode(null);
                      setEditingText("");
                      cm.current?.hide();
                    }
                  }}
                  autoFocus
                  placeholder={
                    editMode === "new-folder" ? "folder name" : "file name"
                  }
                />
                <div className="edit-buttons">
                  <Button
                    icon="pi pi-check"
                    className="p-button-text p-button-sm"
                    onClick={() => {
                      createNewItem(editMode === "new-folder");
                      setEditMode("");
                      setEditingNode(null);
                      setEditingText("");
                      cm.current?.hide();
                    }}
                  />
                  <Button
                    icon="pi pi-times"
                    className="p-button-text p-button-sm"
                    onClick={() => {
                      setEditMode("");
                      setEditingNode(null);
                      setEditingText("");
                      cm.current?.hide();
                    }}
                  />
                </div>
              </div>
            ),
          },
        ];
      }

      return [
        {
          label: "New File",
          icon: "pi pi-file",
          command: (e) => {
            setEditMode("new-file");
            setEditingText("");

            // Hack: Reshow the context menu after a tiny delay
            setTimeout(() => {
              cm.current?.show(e.originalEvent);
            }, 0);
          },
        },
        {
          label: "New Folder",
          icon: "pi pi-folder",
          command: (e) => {
            setEditMode("new-folder");
            setEditingText("");

            // Hack: Reshow the context menu after a tiny delay
            setTimeout(() => {
              cm.current?.show(e.originalEvent);
            }, 0);
          },
        },
      ];
    }

    // Don't allow modifying root index.html
    if (node.data === "index.html") {
      return [];
    }

    const items = [];

    if (
      editMode === "rename" ||
      editMode === "new-file" ||
      editMode === "new-folder"
    ) {
      items.push({
        template: () => (
          <div className="inline-edit" onClick={(e) => e.preventDefault()}>
            <InputText
              value={editingText}
              onChange={(e) => {
                e.stopPropagation();
                setEditingText(e.target.value);
              }}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") {
                  if (editMode === "rename") {
                    handleRename(editingNode, editingText);
                  } else {
                    createNewItem(editMode === "new-folder");
                  }
                  setEditMode("");
                  setEditingNode(null);
                  setEditingText("");
                  cm.current?.hide();
                }
                if (e.key === "Escape") {
                  setEditMode("");
                  setEditingNode(null);
                  setEditingText("");
                  cm.current?.hide();
                }
              }}
              autoFocus
              placeholder={
                editMode === "rename"
                  ? "new name"
                  : editMode === "new-folder"
                    ? "folder name"
                    : "file name"
              }
            />
            <div className="edit-buttons">
              <Button
                icon="pi pi-check"
                className="p-button-text p-button-sm"
                onClick={() => {
                  if (editMode === "rename") {
                    handleRename(editingNode, editingText);
                  } else {
                    createNewItem(editMode === "new-folder");
                  }
                  setEditMode("");
                  setEditingNode(null);
                  setEditingText("");
                  cm.current?.hide();
                }}
              />
              <Button
                icon="pi pi-times"
                className="p-button-text p-button-sm"
                onClick={() => {
                  setEditMode("");
                  setEditingNode(null);
                  setEditingText("");
                  cm.current?.hide();
                }}
              />
            </div>
          </div>
        ),
      });
    } else {
      // Add "New File" and "New Folder" options for folders
      if (node.data.endsWith("/")) {
        items.push(
          {
            label: "New File",
            icon: "pi pi-file",
            command: (e) => {
              setEditMode("new-file");
              setEditingText("");

              // Hack: Reshow the context menu after a tiny delay
              setTimeout(() => {
                cm.current?.show(e.originalEvent);
              }, 0);
            },
          },
          {
            label: "New Folder",
            icon: "pi pi-folder",
            command: (e) => {
              setEditMode("new-folder");
              setEditingText("");

              // Hack: Reshow the context menu after a tiny delay
              setTimeout(() => {
                cm.current?.show(e.originalEvent);
              }, 0);
            },
          },
        );
      }

      items.push(
        {
          label: "Rename",
          icon: "pi pi-pencil",
          command: (e) => {
            setEditMode("rename");
            setEditingNode(node.data);
            // Set initial text to current name without extension for files
            const name = node.data.replace(/\/+$/, "").split("/").pop();
            setEditingText(name);

            // Hack: Reshow the context menu after a tiny delay
            setTimeout(() => {
              cm.current?.show(e.originalEvent);
            }, 0);
          },
        },
        {
          label: "Delete",
          icon: "pi pi-trash",
          className: "p-error",
          command: () => {
            handleDelete(node.data);
            cm.current?.hide();
          },
        },
      );
    }

    return items;
  };

  const handleContextMenu = (e) => {
    const items = getContextMenuItems(e.node);
    if (items.length > 0) {
      cm.current.show(e.originalEvent);
    }
    setEditingNode(e.node ? e.node.data : "");
    setEditingText("");
  };

  const getDefaultHeaders = (filename) => {
    const ext = filename.split(".").pop().toLowerCase();
    const headers = [{ header: "Access-Control-Allow-Origin", value: "*" }];

    const contentTypes = {
      html: "text/html",
      htm: "text/html",
      js: "application/javascript",
      css: "text/css",
      json: "application/json",
      xml: "application/xml",
      txt: "text/plain",
      md: "text/markdown",
      jpg: "image/jpeg",
      jpeg: "image/jpeg",
      png: "image/png",
      gif: "image/gif",
      svg: "image/svg+xml",
      ico: "image/x-icon",
      pdf: "application/pdf",
      zip: "application/zip",
      woff: "font/woff",
      woff2: "font/woff2",
      ttf: "font/ttf",
      eot: "font/eot",
      mp3: "audio/mpeg",
      mp4: "video/mp4",
      webm: "video/webm",
      wav: "audio/wav",
      ogg: "audio/ogg",
      webp: "image/webp",
      csv: "text/csv",
    };

    const contentType = contentTypes[ext] || "text/html";
    headers.push({
      header: "Content-Type",
      value: `${contentType}; charset=utf-8`,
    });

    return headers;
  };

  const createNewItem = (isDirectory) => {
    if (!editingText) {
      setEditingNode(null);
      return;
    }

    const basePath = editingNode || "";
    // Check if the new item name ends with '/' to determine if it's a directory
    const isDir = isDirectory || editingText.endsWith("/");
    const cleanName = editingText.replace(/\/$/, ""); // Remove trailing slash for the name
    const path =
      basePath +
      (basePath.endsWith("/") ? "" : "/") +
      cleanName +
      (isDir ? "/" : "");

    let current = files;
    const parts = path.split("/").filter((p) => p);
    const lastPart = isDir
      ? parts[parts.length - 1] + "/"
      : parts[parts.length - 1];

    for (let i = 0; i < parts.length - 1; i++) {
      const part = parts[i] + "/";
      if (!(part in current)) {
        current[part] = {};
      }
      current = current[part];
    }

    current[lastPart] = isDir
      ? {}
      : {
        raw: "",
        headers: getDefaultHeaders(lastPart),
        status_code: 200,
      };

    onUpdate({ ...files });
    setEditingNode(null);
    setEditingText("");
  };

  const treeNodes = convertToTreeNodes(files);

  const getFileFromPath = (path) => {
    let current = files;
    const parts = path.split("/").filter((p) => p);

    for (let i = 0; i < parts.length - 1; i++) {
      const part = parts[i] + "/";
      if (part in current) {
        current = current[part];
      } else {
        return null;
      }
    }

    const lastPart = parts[parts.length - 1];
    return current[lastPart] || null;
  };

  return (
    <div
      className={`file-tree ${isDark ? "dark" : ""}`}
      onContextMenu={(e) => {
        e.preventDefault();
        cm.current?.show(e);
        setEditingNode("");
        setEditingText("");
        setEditMode("");
      }}
    >
      <ContextMenu
        model={getContextMenuItems(editingNode ? { data: editingNode } : null)}
        ref={cm}
        onHide={() => {
          if (editMode) {
            setEditMode("");
            setEditingNode(null);
            setEditingText("");
          }
        }}
      />
      <Tree
        value={treeNodes}
        selectionMode="single"
        selectionKeys={{ [selectedFile?.path || "index.html"]: true }}
        nodeTemplate={(node) => (
          <span
            className={selectedFile?.path === node.data ? "selected-node" : ""}
          >
            <span className="p-treenode-label">{node.label}</span>
          </span>
        )}
        onSelect={(e) => {
          if (e.node) {
            handleNodeClick(e.node.data);
          }
        }}
        onContextMenu={(e) => {
          e.originalEvent.preventDefault();
          handleContextMenu(e);
        }}
        className={`compact-tree ${isDark ? "dark" : ""}`}
      />
    </div>
  );
};

FileTree.propTypes = {
  files: PropTypes.object,
  selectedFile: PropTypes.object,
  onSelect: PropTypes.func,
  onUpdate: PropTypes.func,
  toast: PropTypes.object,
};

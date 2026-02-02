import { useState, useCallback, useMemo } from "react";
import {
  FilePlus,
  FolderPlus,
  Pencil,
  Trash2,
  File,
  Folder,
} from "lucide-react";
import { toast } from "sonner";
import { ContextMenu, type ContextMenuItem } from "@/components/ui/ContextMenu";
import { FileTreeItem } from "./FileTreeItem";
import {
  buildFileTree,
  type TreeNode,
  isValidFileName,
  isValidFolderName,
  getFileName,
  renameInTree,
  deleteFromTree,
  getFilesInFolder,
  getContentTypeFromExtension,
} from "@/lib/fileTree";
import type { FileTree as FileTreeType } from "@/types";

interface FileTreeProps {
  files: FileTreeType;
  selectedFile: string;
  onSelectFile: (path: string) => void;
  onFilesChange: (files: FileTreeType) => void;
  isLoading?: boolean;
}

export function FileTree({
  files,
  selectedFile,
  onSelectFile,
  onFilesChange,
  isLoading = false,
}: FileTreeProps) {
  // State for expanded folders
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(
    new Set(),
  );

  // State for context menu
  const [contextMenu, setContextMenu] = useState<{
    isOpen: boolean;
    position: { x: number; y: number };
    target: TreeNode | null;
  }>({ isOpen: false, position: { x: 0, y: 0 }, target: null });

  // State for renaming
  const [renamingPath, setRenamingPath] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");

  // State for creating new items
  const [creatingIn, setCreatingIn] = useState<string | null>(null);
  const [creatingType, setCreatingType] = useState<"file" | "folder" | null>(
    null,
  );
  const [newItemName, setNewItemName] = useState("");

  // Build tree structure from flat files
  const tree = useMemo(() => buildFileTree(files), [files]);

  // Toggle folder expanded state
  const handleToggleFolder = useCallback((folderPath: string) => {
    setExpandedFolders((prev) => {
      const next = new Set(prev);
      if (next.has(folderPath)) {
        next.delete(folderPath);
      } else {
        next.add(folderPath);
      }
      return next;
    });
  }, []);

  // Handle file/folder selection
  const handleSelect = useCallback(
    (path: string, isFolder: boolean) => {
      if (!isFolder) {
        onSelectFile(path);
      }
    },
    [onSelectFile],
  );

  // Handle context menu open
  const handleContextMenu = useCallback(
    (e: React.MouseEvent, node: TreeNode) => {
      setContextMenu({
        isOpen: true,
        position: { x: e.clientX, y: e.clientY },
        target: node,
      });
    },
    [],
  );

  // Handle root context menu (empty area)
  const handleRootContextMenu = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setContextMenu({
      isOpen: true,
      position: { x: e.clientX, y: e.clientY },
      target: null, // null = root level
    });
  }, []);

  // Close context menu
  const handleCloseContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, isOpen: false }));
  }, []);

  // Get context menu items based on target
  const getContextMenuItems = useCallback((): ContextMenuItem[] => {
    const target = contextMenu.target;
    const isRoot = target === null;
    const isFolder = target?.type === "folder";
    const isIndexHtml = target?.path === "index.html";

    const items: ContextMenuItem[] = [];

    // New File (available on folders and root)
    if (isRoot || isFolder) {
      items.push({
        key: "new-file",
        label: "New File",
        icon: <FilePlus className="h-4 w-4" />,
      });
      items.push({
        key: "new-folder",
        label: "New Folder",
        icon: <FolderPlus className="h-4 w-4" />,
      });
    }

    // Rename and Delete (not available on root or index.html)
    if (!isRoot) {
      if (items.length > 0) {
        items.push({ key: "divider-1", label: "", divider: true });
      }
      items.push({
        key: "rename",
        label: "Rename",
        icon: <Pencil className="h-4 w-4" />,
        disabled: isIndexHtml,
      });
      items.push({
        key: "delete",
        label: "Delete",
        icon: <Trash2 className="h-4 w-4" />,
        danger: true,
        disabled: isIndexHtml,
      });
    }

    return items;
  }, [contextMenu.target]);

  // Handle delete
  const handleDelete = useCallback(
    (path: string, isFolder: boolean) => {
      if (path === "index.html") {
        toast.error("Cannot delete index.html");
        return;
      }

      if (isFolder) {
        const filesInFolder = getFilesInFolder(files, path);
        if (filesInFolder.length > 0) {
          if (
            !window.confirm(
              `Delete folder "${path}" and ${filesInFolder.length} file(s) inside?`,
            )
          ) {
            return;
          }
        }
      }

      const newFiles = deleteFromTree(files, path);
      onFilesChange(newFiles);

      // If deleted file was selected, select index.html
      if (
        selectedFile === path ||
        (isFolder && selectedFile.startsWith(path + "/"))
      ) {
        onSelectFile("index.html");
      }

      toast.success(isFolder ? "Folder deleted" : "File deleted");
    },
    [files, onFilesChange, selectedFile, onSelectFile],
  );

  // Handle context menu action
  const handleContextMenuAction = useCallback(
    (key: string) => {
      const target = contextMenu.target;
      const isFolder = target?.type === "folder";
      const targetPath = isFolder ? target?.name : target?.path;

      switch (key) {
        case "new-file":
          setCreatingIn(isFolder ? (target?.name ?? "") : "");
          setCreatingType("file");
          setNewItemName("");
          // Expand the folder if creating inside it
          if (isFolder && target?.name) {
            setExpandedFolders((prev) => new Set([...prev, target.name]));
          }
          break;

        case "new-folder":
          setCreatingIn(isFolder ? (target?.name ?? "") : "");
          setCreatingType("folder");
          setNewItemName("");
          if (isFolder && target?.name) {
            setExpandedFolders((prev) => new Set([...prev, target.name]));
          }
          break;

        case "rename":
          if (targetPath) {
            setRenamingPath(targetPath);
            setRenameValue(
              isFolder ? (target?.name ?? "") : getFileName(targetPath),
            );
          }
          break;

        case "delete":
          if (targetPath) {
            handleDelete(targetPath, isFolder ?? false);
          }
          break;
      }
    },
    [contextMenu.target, handleDelete],
  );

  // Handle rename submit
  const handleRenameSubmit = useCallback(() => {
    if (!renamingPath || !renameValue.trim()) {
      setRenamingPath(null);
      return;
    }

    const isFolder = !files[renamingPath]; // If path not in files, it's a folder
    const oldName = isFolder ? renamingPath : getFileName(renamingPath);
    const newName = renameValue.trim();

    if (oldName === newName) {
      setRenamingPath(null);
      return;
    }

    if (!isValidFileName(newName)) {
      toast.error("Invalid name");
      return;
    }

    // Calculate new path
    let newPath: string;
    if (isFolder) {
      newPath = newName;
    } else {
      const parentPath = renamingPath.includes("/")
        ? renamingPath.substring(0, renamingPath.lastIndexOf("/"))
        : "";
      newPath = parentPath ? `${parentPath}/${newName}` : newName;
    }

    // Check for conflicts
    if (files[newPath]) {
      toast.error("A file with this name already exists");
      return;
    }

    const newFiles = renameInTree(files, renamingPath, newPath);
    onFilesChange(newFiles);

    // Update selection if needed
    if (selectedFile === renamingPath) {
      onSelectFile(newPath);
    } else if (selectedFile.startsWith(renamingPath + "/")) {
      const newSelectedPath =
        newPath + selectedFile.substring(renamingPath.length);
      onSelectFile(newSelectedPath);
    }

    setRenamingPath(null);
    toast.success("Renamed successfully");
  }, [
    renamingPath,
    renameValue,
    files,
    onFilesChange,
    selectedFile,
    onSelectFile,
  ]);

  // Handle rename cancel
  const handleRenameCancel = useCallback(() => {
    setRenamingPath(null);
    setRenameValue("");
  }, []);

  // Handle new item creation
  const handleCreateItem = useCallback(() => {
    if (!newItemName.trim() || !creatingType) {
      setCreatingIn(null);
      setCreatingType(null);
      return;
    }

    const name = newItemName.trim();
    const basePath = creatingIn || "";
    const fullPath = basePath ? `${basePath}/${name}` : name;

    if (creatingType === "file") {
      if (!isValidFileName(name)) {
        toast.error("Invalid file name");
        return;
      }

      if (files[fullPath]) {
        toast.error("A file with this name already exists");
        return;
      }

      // Check if creating file would conflict with existing folder
      // e.g., creating "yo" when "yo/index.html" exists
      const existingPaths = Object.keys(files);
      const conflictsWithFolder = existingPaths.some((p) =>
        p.startsWith(fullPath + "/"),
      );
      if (conflictsWithFolder) {
        toast.error("A folder with this name already exists");
        return;
      }

      const newFiles: FileTreeType = {
        ...files,
        [fullPath]: {
          raw: "", // Empty content
          headers: [
            { header: "Access-Control-Allow-Origin", value: "*" },
            {
              header: "Content-Type",
              value: getContentTypeFromExtension(fullPath),
            },
          ],
          status_code: 200,
        },
      };
      onFilesChange(newFiles);
      onSelectFile(fullPath);
      toast.success("File created");
    } else {
      // Creating a folder - create an index.html inside to persist it
      if (!isValidFolderName(name)) {
        toast.error("Invalid folder name");
        return;
      }

      // Check if a file with this exact name exists (can't have both file "yo" and folder "yo/")
      if (files[fullPath]) {
        toast.error("A file with this name already exists");
        return;
      }

      // Check if folder already exists (has files inside)
      const existingPaths = Object.keys(files);
      const folderExists = existingPaths.some((p) =>
        p.startsWith(fullPath + "/"),
      );
      if (folderExists) {
        toast.error("A folder with this name already exists");
        return;
      }

      // Create an index.html file inside the folder
      const indexPath = `${fullPath}/index.html`;
      const newFiles: FileTreeType = {
        ...files,
        [indexPath]: {
          raw: "",
          headers: [
            { header: "Access-Control-Allow-Origin", value: "*" },
            { header: "Content-Type", value: "text/html; charset=utf-8" },
          ],
          status_code: 200,
        },
      };
      onFilesChange(newFiles);
      setExpandedFolders((prev) => new Set([...prev, fullPath]));
      onSelectFile(indexPath);
      toast.success("Folder created with index.html");
    }

    setCreatingIn(null);
    setCreatingType(null);
    setNewItemName("");
  }, [
    newItemName,
    creatingType,
    creatingIn,
    files,
    onFilesChange,
    onSelectFile,
  ]);

  return (
    <div
      className="h-full overflow-auto py-1"
      onContextMenu={handleRootContextMenu}
    >
      {/* Tree items */}
      {tree.map((node) => (
        <FileTreeItem
          key={node.path || node.name}
          node={node}
          depth={0}
          selectedPath={selectedFile}
          expandedFolders={expandedFolders}
          renamingPath={renamingPath}
          renameValue={renameValue}
          creatingIn={creatingIn}
          creatingType={creatingType}
          newItemName={newItemName}
          onSelect={handleSelect}
          onToggleFolder={handleToggleFolder}
          onContextMenu={handleContextMenu}
          onRenameChange={setRenameValue}
          onRenameSubmit={handleRenameSubmit}
          onRenameCancel={handleRenameCancel}
          onNewItemNameChange={setNewItemName}
          onCreateItem={handleCreateItem}
          onCancelCreate={() => {
            setCreatingIn(null);
            setCreatingType(null);
            setNewItemName("");
          }}
        />
      ))}

      {/* New item input (shown at root level) */}
      {creatingType && creatingIn === "" && (
        <div
          className="flex items-center gap-1 px-1 py-0.5"
          style={{ paddingLeft: "4px" }}
        >
          <span className="w-4 h-4" />
          {creatingType === "folder" ? (
            <Folder className="h-4 w-4 text-warning" />
          ) : (
            <File className="h-4 w-4 text-default-500" />
          )}
          <input
            autoFocus
            type="text"
            value={newItemName}
            onChange={(e) => setNewItemName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") handleCreateItem();
              if (e.key === "Escape") {
                setCreatingIn(null);
                setCreatingType(null);
              }
            }}
            onBlur={handleCreateItem}
            placeholder={
              creatingType === "folder" ? "folder name" : "file name"
            }
            className="flex-1 bg-default-100 px-1 py-0 text-sm rounded outline-none focus:ring-1 focus:ring-primary"
          />
        </div>
      )}

      {/* Loading state */}
      {isLoading && tree.length === 0 && (
        <div className="text-center text-default-400 text-sm py-4">
          Loading files...
        </div>
      )}

      {/* Empty state */}
      {!isLoading && tree.length === 0 && !creatingType && (
        <div className="text-center text-default-400 text-sm py-4">
          No files. Right-click to create.
        </div>
      )}

      {/* Context menu */}
      <ContextMenu
        isOpen={contextMenu.isOpen}
        position={contextMenu.position}
        items={getContextMenuItems()}
        onAction={handleContextMenuAction}
        onClose={handleCloseContextMenu}
      />
    </div>
  );
}

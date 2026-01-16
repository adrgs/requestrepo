import { useRef, useEffect } from "react";
import {
  ChevronRight,
  ChevronDown,
  File,
  Folder,
  FolderOpen,
} from "lucide-react";
import { cn } from "@/lib/utils";
import type { TreeNode } from "@/lib/fileTree";

interface FileTreeItemProps {
  node: TreeNode;
  depth: number;
  selectedPath: string | null;
  expandedFolders: Set<string>;
  renamingPath: string | null;
  renameValue: string;
  creatingIn: string | null;
  creatingType: "file" | "folder" | null;
  newItemName: string;
  onSelect: (path: string, isFolder: boolean) => void;
  onToggleFolder: (folderPath: string) => void;
  onContextMenu: (e: React.MouseEvent, node: TreeNode) => void;
  onRenameChange: (value: string) => void;
  onRenameSubmit: () => void;
  onRenameCancel: () => void;
  onNewItemNameChange: (value: string) => void;
  onCreateItem: () => void;
  onCancelCreate: () => void;
}

export function FileTreeItem({
  node,
  depth,
  selectedPath,
  expandedFolders,
  renamingPath,
  renameValue,
  creatingIn,
  creatingType,
  newItemName,
  onSelect,
  onToggleFolder,
  onContextMenu,
  onRenameChange,
  onRenameSubmit,
  onRenameCancel,
  onNewItemNameChange,
  onCreateItem,
  onCancelCreate,
}: FileTreeItemProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const isFolder = node.type === "folder";
  const isExpanded = isFolder && expandedFolders.has(node.name);
  const isSelected =
    node.path === selectedPath || (isFolder && node.name === selectedPath);
  const isRenaming = renamingPath === (isFolder ? node.name : node.path);

  // Focus input when renaming starts
  useEffect(() => {
    if (isRenaming && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [isRenaming]);

  const handleClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (isFolder) {
      onToggleFolder(node.name);
    } else {
      onSelect(node.path, false);
    }
  };

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    onContextMenu(e, node);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter") {
      e.preventDefault();
      onRenameSubmit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onRenameCancel();
    }
  };

  const getIcon = () => {
    if (isFolder) {
      return isExpanded ? (
        <FolderOpen className="h-4 w-4 text-warning" />
      ) : (
        <Folder className="h-4 w-4 text-warning" />
      );
    }
    return <File className="h-4 w-4 text-default-500" />;
  };

  return (
    <div>
      <div
        className={cn(
          "group flex items-center gap-1 px-1 py-0.5 cursor-pointer rounded-md text-sm",
          "hover:bg-default-100",
          isSelected && "bg-primary/10",
        )}
        style={{ paddingLeft: `${depth * 12 + 4}px` }}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        {/* Expand/collapse chevron for folders */}
        <span className="w-4 h-4 flex items-center justify-center shrink-0">
          {isFolder && (
            <button
              className="hover:bg-default-200 rounded"
              onClick={(e) => {
                e.stopPropagation();
                onToggleFolder(node.name);
              }}
            >
              {isExpanded ? (
                <ChevronDown className="h-3.5 w-3.5" />
              ) : (
                <ChevronRight className="h-3.5 w-3.5" />
              )}
            </button>
          )}
        </span>

        {/* Icon */}
        <span className="shrink-0">{getIcon()}</span>

        {/* Name or rename input */}
        {isRenaming ? (
          <input
            ref={inputRef}
            type="text"
            value={renameValue}
            onChange={(e) => onRenameChange(e.target.value)}
            onKeyDown={handleKeyDown}
            onBlur={onRenameSubmit}
            className="flex-1 bg-default-100 px-1 py-0 text-sm rounded outline-none focus:ring-1 focus:ring-primary"
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span className="truncate">{node.name}</span>
        )}
      </div>

      {/* Children (for folders) */}
      {isFolder &&
        isExpanded &&
        (node.children.length > 0 || creatingIn === node.name) && (
          <div>
            {node.children.map((child) => (
              <FileTreeItem
                key={child.path || child.name}
                node={child}
                depth={depth + 1}
                selectedPath={selectedPath}
                expandedFolders={expandedFolders}
                renamingPath={renamingPath}
                renameValue={renameValue}
                creatingIn={creatingIn}
                creatingType={creatingType}
                newItemName={newItemName}
                onSelect={onSelect}
                onToggleFolder={onToggleFolder}
                onContextMenu={onContextMenu}
                onRenameChange={onRenameChange}
                onRenameSubmit={onRenameSubmit}
                onRenameCancel={onRenameCancel}
                onNewItemNameChange={onNewItemNameChange}
                onCreateItem={onCreateItem}
                onCancelCreate={onCancelCreate}
              />
            ))}
            {/* New item input inside this folder */}
            {creatingType && creatingIn === node.name && (
              <div
                className="flex items-center gap-1 px-1 py-0.5"
                style={{ paddingLeft: `${(depth + 1) * 12 + 4}px` }}
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
                  onChange={(e) => onNewItemNameChange(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") onCreateItem();
                    if (e.key === "Escape") onCancelCreate();
                  }}
                  onBlur={onCreateItem}
                  placeholder={
                    creatingType === "folder" ? "folder name" : "file name"
                  }
                  className="flex-1 bg-default-100 px-1 py-0 text-sm rounded outline-none focus:ring-1 focus:ring-primary"
                />
              </div>
            )}
          </div>
        )}
    </div>
  );
}

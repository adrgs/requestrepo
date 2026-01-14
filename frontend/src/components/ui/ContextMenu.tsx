import { useEffect, useRef, useCallback } from "react";
import { createPortal } from "react-dom";
import { cn } from "@/lib/utils";

export interface ContextMenuItem {
  key: string;
  label: string;
  icon?: React.ReactNode;
  disabled?: boolean;
  danger?: boolean;
  divider?: boolean;
}

interface ContextMenuProps {
  isOpen: boolean;
  position: { x: number; y: number };
  items: ContextMenuItem[];
  onAction: (key: string) => void;
  onClose: () => void;
}

export function ContextMenu({
  isOpen,
  position,
  items,
  onAction,
  onClose,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  // Adjust position to stay within viewport
  const getAdjustedPosition = useCallback(() => {
    if (!menuRef.current) return position;

    const menu = menuRef.current;
    const rect = menu.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;

    let { x, y } = position;

    // Adjust horizontal position
    if (x + rect.width > viewportWidth - 8) {
      x = viewportWidth - rect.width - 8;
    }

    // Adjust vertical position
    if (y + rect.height > viewportHeight - 8) {
      y = viewportHeight - rect.height - 8;
    }

    return { x: Math.max(8, x), y: Math.max(8, y) };
  }, [position]);

  // Handle click outside
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };

    const handleScroll = () => {
      onClose();
    };

    // Use capture phase to catch clicks before other handlers
    document.addEventListener("mousedown", handleClickOutside, true);
    document.addEventListener("keydown", handleEscape);
    window.addEventListener("scroll", handleScroll, true);

    return () => {
      document.removeEventListener("mousedown", handleClickOutside, true);
      document.removeEventListener("keydown", handleEscape);
      window.removeEventListener("scroll", handleScroll, true);
    };
  }, [isOpen, onClose]);

  // Reposition after render
  useEffect(() => {
    if (isOpen && menuRef.current) {
      const adjusted = getAdjustedPosition();
      menuRef.current.style.left = `${adjusted.x}px`;
      menuRef.current.style.top = `${adjusted.y}px`;
    }
  }, [isOpen, getAdjustedPosition]);

  if (!isOpen) return null;

  const menu = (
    <div
      ref={menuRef}
      className="fixed z-[9999] min-w-[160px] rounded-lg bg-content1 p-1 shadow-lg border border-default-200 dark:border-default-100"
      style={{ left: position.x, top: position.y }}
    >
      {items.map((item) => {
        if (item.divider) {
          return (
            <div
              key={item.key}
              className="my-1 h-px bg-default-200 dark:bg-default-100"
            />
          );
        }

        return (
          <button
            key={item.key}
            className={cn(
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-sm text-left transition-colors",
              item.disabled
                ? "cursor-not-allowed opacity-50"
                : item.danger
                ? "hover:bg-danger/10 hover:text-danger"
                : "hover:bg-default-100"
            )}
            disabled={item.disabled}
            onClick={() => {
              if (!item.disabled) {
                onAction(item.key);
                onClose();
              }
            }}
          >
            {item.icon && <span className="w-4 h-4">{item.icon}</span>}
            <span>{item.label}</span>
          </button>
        );
      })}
    </div>
  );

  return createPortal(menu, document.body);
}

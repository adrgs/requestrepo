import { useState, useCallback, useEffect, useRef } from "react";
import {
  Button,
  Input,
  Autocomplete,
  AutocompleteItem,
  Card,
  CardBody,
  Select,
  SelectItem,
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
} from "@heroui/react";
import {
  Plus,
  Trash2,
  Save,
  ChevronDown,
  ChevronUp,
  FolderOpen,
  FilePlus,
} from "lucide-react";
import { toast } from "sonner";
import Editor from "@monaco-editor/react";
import { useSessionStore } from "@/stores/sessionStore";
import { useTheme } from "@/hooks/useTheme";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiClient } from "@/api/client";
import { encodeBase64, decodeBase64 } from "@/lib/base64";
import { FileTree } from "@/components/file-tree";
import type { FileTree as FileTreeType, ResponseHeader } from "@/types";

// Common HTTP headers for autocomplete
const HTTP_HEADERS = [
  "Accept",
  "Accept-CH",
  "Accept-Charset",
  "Accept-Encoding",
  "Accept-Language",
  "Accept-Patch",
  "Accept-Ranges",
  "Access-Control-Allow-Credentials",
  "Access-Control-Allow-Headers",
  "Access-Control-Allow-Methods",
  "Access-Control-Allow-Origin",
  "Access-Control-Expose-Headers",
  "Access-Control-Max-Age",
  "Access-Control-Request-Headers",
  "Access-Control-Request-Method",
  "Age",
  "Allow",
  "Alt-Svc",
  "Authorization",
  "Cache-Control",
  "Clear-Site-Data",
  "Connection",
  "Content-Disposition",
  "Content-Encoding",
  "Content-Language",
  "Content-Length",
  "Content-Location",
  "Content-Range",
  "Content-Security-Policy",
  "Content-Type",
  "Cookie",
  "Cross-Origin-Embedder-Policy",
  "Cross-Origin-Opener-Policy",
  "Cross-Origin-Resource-Policy",
  "Date",
  "ETag",
  "Expect",
  "Expires",
  "Forwarded",
  "From",
  "Host",
  "If-Match",
  "If-Modified-Since",
  "If-None-Match",
  "If-Range",
  "If-Unmodified-Since",
  "Keep-Alive",
  "Last-Modified",
  "Link",
  "Location",
  "Max-Forwards",
  "Origin",
  "Pragma",
  "Proxy-Authenticate",
  "Proxy-Authorization",
  "Range",
  "Referer",
  "Referrer-Policy",
  "Retry-After",
  "Server",
  "Set-Cookie",
  "Strict-Transport-Security",
  "TE",
  "Trailer",
  "Transfer-Encoding",
  "Upgrade",
  "User-Agent",
  "Vary",
  "Via",
  "WWW-Authenticate",
  "Warning",
  "X-Content-Type-Options",
  "X-DNS-Prefetch-Control",
  "X-Frame-Options",
  "X-Forwarded-For",
  "X-Forwarded-Host",
  "X-Forwarded-Proto",
  "X-Powered-By",
  "X-Requested-With",
  "X-XSS-Protection",
];

// Get Monaco language from filename
function getLanguage(filename: string): string {
  const ext = filename.split(".").pop()?.toLowerCase();
  switch (ext) {
    case "json":
      return "json";
    case "js":
      return "javascript";
    case "ts":
      return "typescript";
    case "css":
      return "css";
    case "html":
    case "htm":
      return "html";
    case "md":
      return "markdown";
    case "xml":
      return "xml";
    default:
      return "plaintext";
  }
}

export function ResponseEditorPage() {
  const { resolvedTheme } = useTheme();
  const sessions = useSessionStore((s) => s.sessions);
  const activeSubdomain = useSessionStore((s) => s.activeSubdomain);
  const session = sessions.find((s) => s.subdomain === activeSubdomain);
  const queryClient = useQueryClient();

  const [selectedFile, setSelectedFile] = useState<string>("index.html");
  const [content, setContent] = useState<string>("");
  const [statusCode, setStatusCode] = useState<string>("200");
  const [headers, setHeaders] = useState<ResponseHeader[]>([
    { header: "Content-Type", value: "text/html" },
  ]);
  const [settingsExpanded, setSettingsExpanded] = useState(true);
  const [newFileModalOpen, setNewFileModalOpen] = useState(false);
  const [newFileName, setNewFileName] = useState("");
  const handleSaveRef = useRef<() => void>(() => {});

  const { data: files = {}, isLoading: isLoadingFiles } = useQuery({
    queryKey: ["files", session?.token],
    queryFn: () => apiClient.getFiles(session!.token),
    enabled: Boolean(session?.token),
  });

  // Track the last loaded file to avoid re-syncing unnecessarily
  const lastLoadedRef = useRef<{ file: string; raw: string } | null>(null);

  // Initialize content when files load or selected file changes
  useEffect(() => {
    const fileData = files[selectedFile];
    if (fileData) {
      // Only sync if file changed or content from server changed
      const shouldSync =
        !lastLoadedRef.current ||
        lastLoadedRef.current.file !== selectedFile ||
        lastLoadedRef.current.raw !== fileData.raw;

      if (shouldSync) {
        lastLoadedRef.current = { file: selectedFile, raw: fileData.raw };
        // Use callback form to batch updates
        queueMicrotask(() => {
          setContent(decodeBase64(fileData.raw));
          setStatusCode(String(fileData.status_code));
          setHeaders(fileData.headers);
        });
      }
    }
  }, [files, selectedFile]);

  const updateFilesMutation = useMutation({
    mutationFn: (newFiles: FileTreeType) =>
      apiClient.updateFiles(session!.token, newFiles),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["files", session?.token] });
      toast.success("Saved");
    },
    onError: () => {
      toast.error("Failed to save");
    },
  });

  const handleSave = useCallback(() => {
    const code = parseInt(statusCode, 10);
    if (isNaN(code) || code < 100 || code > 599) {
      toast.error("Status code must be between 100 and 599");
      return;
    }
    const newFiles: FileTreeType = {
      ...files,
      [selectedFile]: {
        raw: encodeBase64(content),
        status_code: code,
        headers,
      },
    };
    updateFilesMutation.mutate(newFiles);
  }, [files, selectedFile, content, statusCode, headers, updateFilesMutation]);

  // Keep ref updated for keyboard shortcut
  useEffect(() => {
    handleSaveRef.current = handleSave;
  }, [handleSave]);

  // Global Ctrl/Cmd+S handler (works everywhere on the page)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        e.stopPropagation();
        handleSaveRef.current();
      }
    };

    // Use capture phase to intercept before browser handles it
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, []);

  // Handle file tree changes from FileTree component
  const handleFilesChange = useCallback(
    (newFiles: FileTreeType) => {
      updateFilesMutation.mutate(newFiles);
    },
    [updateFilesMutation],
  );

  const handleAddHeader = useCallback(() => {
    setHeaders((prev) => [...prev, { header: "", value: "" }]);
  }, []);

  const handleRemoveHeader = useCallback((index: number) => {
    setHeaders((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const handleHeaderChange = useCallback(
    (index: number, field: "header" | "value", value: string) => {
      setHeaders((prev) =>
        prev.map((h, i) => (i === index ? { ...h, [field]: value } : h)),
      );
    },
    [],
  );

  const handleOpenNewFileModal = useCallback(() => {
    setNewFileName("");
    setNewFileModalOpen(true);
  }, []);

  const handleCreateFile = useCallback(() => {
    const trimmed = newFileName.trim();
    if (!trimmed) {
      toast.error("Filename cannot be empty");
      return;
    }
    if (files[trimmed]) {
      toast.error("File already exists");
      return;
    }

    const newFiles: FileTreeType = {
      ...files,
      [trimmed]: {
        raw: encodeBase64(""),
        status_code: 200,
        headers: [{ header: "Content-Type", value: "text/html" }],
      },
    };
    updateFilesMutation.mutate(newFiles, {
      onSuccess: () => {
        setSelectedFile(trimmed);
        setNewFileModalOpen(false);
        setNewFileName("");
      },
    });
  }, [newFileName, files, updateFilesMutation]);

  if (!session) {
    return <div className="p-4 text-default-500">No session selected</div>;
  }

  const fileList = Object.keys(files);

  return (
    <Card className="h-full">
      <CardBody className="flex h-full flex-col overflow-hidden p-0 md:flex-row">
        {/* Main Content: Editor + Settings */}
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          {/* Mobile: Top bar with file selector and save */}
          <div className="flex shrink-0 items-center gap-2 border-b border-default-200 p-2 md:hidden">
            <FolderOpen className="h-4 w-4 shrink-0 text-default-400" />
            <Select
              size="sm"
              selectedKeys={[selectedFile]}
              onSelectionChange={(keys) => {
                const selected = Array.from(keys)[0] as string;
                if (selected) setSelectedFile(selected);
              }}
              className="flex-1"
              classNames={{
                trigger: "h-8 min-h-8",
                value: "font-mono text-sm",
              }}
              aria-label="Select file"
            >
              {fileList.map((file) => (
                <SelectItem key={file} textValue={file}>
                  <span className="font-mono text-sm">{file}</span>
                </SelectItem>
              ))}
            </Select>
            <Button
              size="sm"
              isIconOnly
              variant="flat"
              onPress={handleOpenNewFileModal}
            >
              <FilePlus className="h-4 w-4" />
            </Button>
            <Button
              color="primary"
              size="sm"
              isIconOnly
              onPress={handleSave}
              isLoading={updateFilesMutation.isPending}
            >
              <Save className="h-4 w-4" />
            </Button>
          </div>

          {/* Editor */}
          <div className="min-h-[200px] flex-[6] md:min-h-0">
            <Editor
              height="100%"
              language={getLanguage(selectedFile)}
              theme={resolvedTheme === "dark" ? "vs-dark" : "light"}
              value={content}
              onChange={(value) => setContent(value ?? "")}
              options={{
                minimap: { enabled: false },
                fontSize: 13,
                lineNumbers: "on",
                wordWrap: "on",
                automaticLayout: true,
                scrollBeyondLastLine: false,
                padding: { top: 8 },
              }}
            />
          </div>

          {/* Response Settings */}
          <div className="flex-[4] overflow-auto border-t border-default-200 md:border-t-0">
            {/* Settings Header - collapsible on mobile */}
            <div
              className="flex cursor-pointer items-center justify-between p-3 md:cursor-default md:p-4"
              onClick={() => setSettingsExpanded(!settingsExpanded)}
            >
              <div className="flex items-center gap-2">
                <h3 className="text-sm font-medium">Response Settings</h3>
                <button className="text-default-400 md:hidden">
                  {settingsExpanded ? (
                    <ChevronUp className="h-4 w-4" />
                  ) : (
                    <ChevronDown className="h-4 w-4" />
                  )}
                </button>
              </div>
              {/* Desktop save button */}
              <Button
                color="primary"
                size="sm"
                startContent={<Save className="h-4 w-4" />}
                onPress={handleSave}
                isLoading={updateFilesMutation.isPending}
                className="hidden md:flex"
              >
                Save
              </Button>
            </div>

            {/* Settings Content - collapsible on mobile */}
            <div
              className={`space-y-4 px-3 pb-3 md:px-4 md:pb-4 ${
                settingsExpanded ? "block" : "hidden md:block"
              }`}
            >
              {/* Status Code */}
              <div className="flex flex-col gap-2 md:flex-row md:items-center md:gap-4">
                <label className="text-sm text-default-500 md:w-24">
                  Status Code
                </label>
                <Input
                  type="number"
                  size="sm"
                  value={statusCode}
                  onValueChange={setStatusCode}
                  min={100}
                  max={599}
                  className="w-full md:w-32"
                  classNames={{
                    input: "font-mono",
                  }}
                />
              </div>

              {/* Headers */}
              <div className="flex flex-col gap-2 md:flex-row md:items-start md:gap-4">
                <label className="text-sm text-default-500 md:w-24 md:pt-2">
                  Headers
                </label>
                <div className="flex-1">
                  <div className="space-y-3">
                    {headers.map((header, index) => (
                      <div
                        key={index}
                        className="flex flex-col gap-2 rounded-lg bg-default-50 p-2 md:flex-row md:items-center md:bg-transparent md:p-0"
                      >
                        <Autocomplete
                          size="sm"
                          placeholder="Header name"
                          defaultItems={HTTP_HEADERS.map((h) => ({ value: h }))}
                          inputValue={header.header}
                          onInputChange={(value) =>
                            handleHeaderChange(index, "header", value)
                          }
                          onSelectionChange={(key) => {
                            if (key)
                              handleHeaderChange(index, "header", String(key));
                          }}
                          className="w-full md:w-64 lg:w-80"
                          classNames={{
                            base: "font-mono text-sm",
                          }}
                          allowsCustomValue
                        >
                          {(item) => (
                            <AutocompleteItem key={item.value}>
                              {item.value}
                            </AutocompleteItem>
                          )}
                        </Autocomplete>
                        <div className="flex items-center gap-2">
                          <Input
                            size="sm"
                            placeholder="Value"
                            value={header.value}
                            onValueChange={(v) =>
                              handleHeaderChange(index, "value", v)
                            }
                            className="flex-1"
                            classNames={{
                              input: "font-mono text-sm",
                            }}
                          />
                          <Button
                            isIconOnly
                            size="sm"
                            variant="light"
                            color="danger"
                            onPress={() => handleRemoveHeader(index)}
                          >
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        </div>
                      </div>
                    ))}
                  </div>
                  <Button
                    size="sm"
                    variant="flat"
                    startContent={<Plus className="h-4 w-4" />}
                    onPress={handleAddHeader}
                    className="mt-3"
                  >
                    Add Header
                  </Button>
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Right: File Explorer - Desktop only */}
        <div className="hidden w-56 shrink-0 flex-col bg-default-50 dark:bg-zinc-900/50 md:flex">
          {/* Explorer Header */}
          <div className="flex items-center px-3 py-2 text-xs font-medium uppercase text-default-500">
            <span>Files</span>
            <span className="ml-1 text-default-400">
              (right-click for menu)
            </span>
          </div>

          {/* File Tree */}
          <FileTree
            files={files}
            selectedFile={selectedFile}
            onSelectFile={setSelectedFile}
            onFilesChange={handleFilesChange}
            isLoading={isLoadingFiles}
          />
        </div>
      </CardBody>

      {/* New File Modal */}
      <Modal
        isOpen={newFileModalOpen}
        onOpenChange={setNewFileModalOpen}
        size="sm"
      >
        <ModalContent>
          {(onClose) => (
            <form
              onSubmit={(e) => {
                e.preventDefault();
                handleCreateFile();
              }}
            >
              <ModalHeader>Create New File</ModalHeader>
              <ModalBody>
                <Input
                  label="Filename"
                  placeholder="e.g., page.html, api.json"
                  value={newFileName}
                  onValueChange={setNewFileName}
                  autoFocus
                  classNames={{
                    input: "font-mono",
                  }}
                />
              </ModalBody>
              <ModalFooter>
                <Button variant="flat" onPress={onClose}>
                  Cancel
                </Button>
                <Button
                  color="primary"
                  type="submit"
                  isDisabled={!newFileName.trim()}
                  isLoading={updateFilesMutation.isPending}
                  startContent={<FilePlus className="h-4 w-4" />}
                >
                  Create
                </Button>
              </ModalFooter>
            </form>
          )}
        </ModalContent>
      </Modal>
    </Card>
  );
}

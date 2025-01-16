import React, { useState, useEffect, useRef } from "react";
import { Button } from "primereact/button";
import { InputText } from "primereact/inputtext";
import { HeaderInput } from "./header-input";
import { Utils } from "../utils";
import { EditorComponent } from "./editor";
import { HeaderService } from "../header-service";
import "react-toastify/dist/ReactToastify.css";
import { FileTree } from "./file-tree";

export const EditResponsePage = ({
  headers: propHeaders = [],
  content: propContent = "",
  fetched: propFetched = false,
  statusCode: propStatusCode = 200,
  toast,
  user,
}) => {
  const [headers, setHeaders] = useState(propHeaders);
  const [content, setContent] = useState(propContent);
  const [fetched, setFetched] = useState(propFetched);
  const [statusCode, setStatusCode] = useState(propStatusCode);
  const [headersData, setHeadersData] = useState([]);
  const fileInput = useRef(null);
  const [files, setFiles] = useState({ items: [] });
  const [selectedFile, setSelectedFile] = useState(null);

  const commands = [
    {
      name: "save",
      exec: () => {
        saveChanges();
      },
    },
  ];

  const handleEditorChange = (value) => {
    setContent(value);
    if (selectedFile) {
      const updatedFiles = { ...files };
      let current = updatedFiles;
      const parts = selectedFile.path.split("/").filter((p) => p);

      for (let i = 0; i < parts.length - 1; i++) {
        current = current[parts[i] + "/"];
      }

      current[parts[parts.length - 1]] = {
        ...current[parts[parts.length - 1]],
        raw: Utils.base64Encode(value),
      };

      setFiles(updatedFiles);
    }
  };
  // Reset state when user changes
  useEffect(() => {
    if (!user) return;

    setContent("");
    setHeaders([]);
    setStatusCode(200);
    setFiles({});
    setSelectedFile(null);
    setFetched(false);

    return () => {
      // Cleanup when user changes or component unmounts
    };
  }, [user]);

  // Fetch data when component mounts or user changes
  useEffect(() => {
    const fetchData = async () => {
      if (!user?.subdomain) {
        return;
      }

      try {
        // Get token for this subdomain
        const token = Utils.getSessionToken(user.subdomain);
        if (!token) {
          throw new Error('No valid token found for session');
        }

        const files = await Utils.getFiles(user.subdomain);
        if (files["index.html"]) {
          setHeaders(files["index.html"].headers);
          setContent(Utils.base64Decode(files["index.html"].raw));
          setStatusCode(files["index.html"].status_code || 200);
          setSelectedFile({
            path: "index.html",
            ...files["index.html"],
          });
        }
        setFiles(files);
        setFetched(true);
      } catch (error) {
        const msg = error.response?.status === 403 
          ? "Your session token is invalid. Please request a new URL"
          : error.message || "Failed to fetch files";
        
        toast.error(msg, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        });
      }
    };

    fetchData();
  }, [user?.subdomain]);
  // Fetch headers separately
  useEffect(() => {
    if (!user) return;

    const fetchHeaders = async () => {
      try {
        const data = await new HeaderService().getHeaders();
        setHeadersData(data);
      } catch (error) {
        toast.error("Failed to fetch headers", {
          position: "bottom-center",
          autoClose: 4000,
          dark: Utils.isDarkTheme()
        });
      }
    };

    fetchHeaders();
  }, []);

  const handleFileUpdate = async (newFiles) => {
    if (!user) {
      toast.error("No active session");
      return;
    }
    try {
      await Utils.updateFiles(newFiles, user.token, user.subdomain);
      setFiles(newFiles);
      toast.success("Files updated successfully!", {
        position: "bottom-center",
        autoClose: 4000,
        hideProgressBar: false,
        closeOnClick: true,
        pauseOnHover: true,
        draggable: true,
        dark: Utils.isDarkTheme(),
      });
    } catch (error) {
      const errorMsg = error.response?.status === 403 ? 
        "Session token is invalid. Please request a new URL" : 
        "Failed to update files";
      toast.error(errorMsg, {
        position: "bottom-center",
        autoClose: 4000,
        hideProgressBar: false,
        closeOnClick: true,
        pauseOnHover: true,
        draggable: true,
        dark: Utils.isDarkTheme(),
      });
    }
  };

  const handleFileUpload = () => {
    fileInput.current.click();
  };

  const handleFileChange = (event) => {
    const file = event.target.files[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = (e) => {
      setContent(Utils.arrayBufferToString(e.target.result));
    };
    reader.readAsArrayBuffer(file);
  };

  const handleFileSelect = (file) => {
    if (!user) {
      toast.error("No active session");
      return;
    }
    setSelectedFile(file);
    setContent(Utils.base64Decode(file.raw));
    setHeaders(file.headers || []);
    setStatusCode(file.status_code || 200);
  };

  const saveChanges = async () => {
    if (!user?.subdomain) {
      toast.error("No active session");
      return;
    }

    // Check for valid token
    const token = Utils.getSessionToken(user.subdomain);
    if (!token) {
      toast.error("Invalid session token");
      return;
    }

    try {
      const updatedFiles = { ...files };
      if (selectedFile) {
        let current = updatedFiles;
        const parts = selectedFile.path.split("/").filter((p) => p);

        for (let i = 0; i < parts.length - 1; i++) {
          current = current[parts[i] + "/"];
        }

        current[parts[parts.length - 1]] = {
          raw: Utils.base64Encode(content),
          headers: headers.filter((h) => h.header.length > 0),
          status_code: Number(statusCode) || 200,
        };

        await Utils.updateFiles(updatedFiles, user.subdomain);
        
        toast.success("Changes saved successfully", {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        });
      }
    } catch (error) {
      console.error("Error saving changes:", error);
      toast.error("Failed to save changes", {
        position: "bottom-center",
        autoClose: 4000,
        hideProgressBar: false,
        closeOnClick: true,
        pauseOnHover: true,
        draggable: true,
        dark: Utils.isDarkTheme(),
      });
    }
  };

  const addHeader = () => {
    if (!user) {
      toast.error("No active session");
      return;
    }
    setHeaders([...headers, { header: "", value: "" }]);
  };

  const handleHeaderInputChange = (index, header, value, toDelete) => {
    const updatedHeaders = [...headers];
    if (toDelete) {
      updatedHeaders.splice(index, 1);
    } else {
      updatedHeaders[index] = { header, value };
    }
    setHeaders(updatedHeaders);
  };

  const getLanguageFromPath = (path) => {
    if (!path) return "html";
    const ext = path.split(".").pop().toLowerCase();

    const languageMap = {
      // Web
      html: "html",
      htm: "html",
      js: "javascript",
      jsx: "javascript",
      ts: "typescript",
      tsx: "typescript",
      css: "css",
      scss: "scss",
      less: "less",
      json: "json",
      xml: "xml",
      svg: "xml",

      // Programming
      py: "python",
      rb: "ruby",
      php: "php",
      java: "java",
      cpp: "cpp",
      c: "c",
      cs: "csharp",
      go: "go",
      rs: "rust",
      swift: "swift",
      kt: "kotlin",

      // Config & Data
      yml: "yaml",
      yaml: "yaml",
      toml: "toml",
      ini: "ini",
      md: "markdown",
      sql: "sql",
      sh: "shell",
      bash: "shell",

      // Others
      txt: "plaintext",
      log: "plaintext",
      conf: "plaintext",
    };

    return languageMap[ext] || "plaintext";
  };

  return (
    <div className="card card-w-title card-body">
      <div className="grid">
        <div className="col-12">
          <div className="grid">
            <div className="col-6">
              <h1>Edit Response</h1>
            </div>
            <div className="col-6">
              <Button
                label="Save changes"
                icon="pi pi-save"
                className="p-button-text p-button-success"
                style={{ float: "right" }}
                onClick={saveChanges}
              />
              <Button
                label="Upload file"
                icon="pi pi-upload"
                className="p-button-text"
                style={{ float: "right" }}
                onClick={handleFileUpload}
              />
              <input
                type="file"
                id="file"
                ref={fileInput}
                onChange={handleFileChange}
                style={{ display: "none" }}
              />
            </div>
          </div>
          <div className="grid">
            <div className="col-10">
              <EditorComponent
                value={content}
                onChange={handleEditorChange}
                commands={commands}
                language={
                  selectedFile ? getLanguageFromPath(selectedFile.path) : "html"
                }
              />
            </div>
            <div className="col-2" style={{ height: "416px" }}>
              <FileTree
                files={files}
                selectedFile={selectedFile}
                onSelect={handleFileSelect}
                onUpdate={handleFileUpdate}
                toast={toast}
              />
            </div>
          </div>
          <h1>Status Code</h1>
          <InputText
            value={statusCode}
            onChange={(e) => {
              if (
                e.target.value.length < 10 &&
                /^[0-9]*$/.test(e.target.value)
              ) {
                setStatusCode(e.target.value);
              }
            }}
          />
        </div>
        <div className="col-12">
          <div className="grid">
            <div className="col-6">
              <h1>Response HTTP Headers</h1>
            </div>
            <div className="col-6">
              <Button
                label="Add header"
                onClick={addHeader}
                icon="pi pi-plus"
                className="p-button-text"
                style={{ float: "right", top: "-3px" }}
              />
            </div>
          </div>
          <div>
            {headers.map((element, index) => (
              <HeaderInput
                key={index}
                index={index}
                header={element.header}
                value={element.value}
                handleHeaderInputChange={handleHeaderInputChange}
                headersData={headersData}
              />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};

export default EditResponsePage;

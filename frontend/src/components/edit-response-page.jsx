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

  useEffect(() => {
    const fetchData = async () => {
      try {
        const files = await Utils.getFiles();
        // Load index.html by default
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
        let msg = "Failed to fetch files";
        if (error.response?.status === 403) {
          msg = "Your session token is invalid. Please request a new URL";
        }
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

    if (!fetched) {
      fetchData();
    }

    const fetchHeaders = async () => {
      try {
        const data = await new HeaderService().getHeaders();
        setHeadersData(data);
      } catch (error) {
        console.error("Failed to fetch headers");
      }
    };

    fetchHeaders();
  }, [fetched, toast]);

  const handleFileUpdate = async (newFiles) => {
    try {
      await Utils.updateFiles(newFiles);
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
      toast.error("Failed to update files", {
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
    setSelectedFile(file);
    setContent(Utils.base64Decode(file.raw));
    setHeaders(file.headers || []);
    setStatusCode(file.status_code || 200);
  };

  const saveChanges = () => {
    if (selectedFile) {
      const updatedFiles = { ...files };
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

      handleFileUpdate(updatedFiles)
        .then(() => {})
        .catch((error) => {
          console.error("Error saving file:", error);
          toast.error("Failed to save file.", {
            position: "bottom-center",
            autoClose: 4000,
            hideProgressBar: false,
            closeOnClick: true,
            pauseOnHover: true,
            draggable: true,
            dark: Utils.isDarkTheme(),
          });
        });
    }
  };

  const addHeader = () => {
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

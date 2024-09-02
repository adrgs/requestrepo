import React, { useState, useEffect, useRef, useCallback } from "react";
import { Button } from "primereact/button";
import { InputText } from "primereact/inputtext";
import { HeaderInput } from "./header-input";
import { Utils } from "../utils";
import { EditorComponent } from "./editor";
import { HeaderService } from "../header-service";
import "react-toastify/dist/ReactToastify.css";

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
  };

  useEffect(() => {
    const fetchData = async () => {
      try {
        const res = await Utils.getFile();
        if (!("headers" in res)) throw new Error("Invalid response");
        setHeaders(res["headers"]);
        try {
          setContent(Utils.base64Decode(res["raw"]));
        } catch {
          console.error("Failed to decode base64 content");
        }
        setStatusCode(res["status_code"]);
        setFetched(true);
      } catch (error) {
        let msg = "Failed to fetch file";
        if (error.response.status === 403) {
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

  const saveChanges = () => {
    const filteredHeaders = headers.filter((value) => value.header.length > 0);
    const obj = {
      headers: filteredHeaders,
      status_code: statusCode,
      raw: Utils.base64Encode(content),
    };

    Utils.updateFile(obj).then((res) => {
      if (res.error) {
        toast.error(res.error, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        });
      } else {
        toast.success(res.msg, {
          position: "bottom-center",
          autoClose: 4000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          dark: Utils.isDarkTheme(),
        });
        Utils.getFile().then((res) => {
          setHeaders(res["headers"]);
          try {
            setContent(Utils.base64Decode(res["raw"]));
          } catch {
            console.error("Failed to decode base64 content");
          }
          setStatusCode(res["status_code"]);
          setFetched(true);
        });
      }
    }).catch((error) => {
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
          <EditorComponent
            value={content}
            onChange={handleEditorChange}
            commands={commands}
            language={"html"}
          />
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

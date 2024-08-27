import React, { useEffect, useRef, useState } from "react";
import MonacoEditor from "@monaco-editor/react";
import { Utils } from "../utils";

export const EditorComponent = ({ value, onChange, commands, language, onFocus, onBlur }) => {
  const editorRef = useRef(null);
  const monacoRef = useRef(null);
  const [theme, setTheme] = useState(Utils.getTheme() === "dark" ? "vs-dark" : "vs-light");

  useEffect(() => {
    const handleThemeChange = () => {
      setTheme(Utils.getTheme() === "dark" ? "vs-dark" : "vs-light");
    };

    // Assume there's an event or method to detect theme changes
    // You need to implement this part based on how your application handles theme changes
    window.addEventListener("themeChange", handleThemeChange);

    return () => {
      window.removeEventListener("themeChange", handleThemeChange);
    };
  }, []);

  useEffect(() => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    if (!editor || !monaco) return;
    commands.forEach((command) => {
      editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, command.exec);
    });
  }, [commands]);

  const handleEditorDidMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;

    // Implement focus and blur events
    if (onFocus) {
      editor.onDidFocusEditorWidget(() => {
        onFocus();
      });
    }

    if (onBlur) {
      editor.onDidBlurEditorWidget(() => {
        onBlur();
      });
    }
  };

  return (
    <MonacoEditor
      width="100%"
      height="400px"
      language={language}
      theme={theme}
      value={value}
      options={{
        fontSize: 14,
        scrollBeyondLastLine: false,
        minimap: { enabled: false },
        automaticLayout: true,
        tabSize: 2,
        wordWrap: "on",
      }}
      onChange={onChange}
      onMount={handleEditorDidMount}
    />
  );
};

export default EditorComponent;

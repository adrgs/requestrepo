import React, { useEffect, useRef, useState } from "react";
import MonacoEditor, { Monaco, OnMount } from "@monaco-editor/react";
import { Utils } from "../utils";

interface EditorCommand {
  exec: () => void;
}

interface EditorComponentProps {
  value: string;
  onChange: (value: string) => void;
  commands: EditorCommand[];
  language: string;
  onFocus?: () => void;
  onBlur?: () => void;
}

export const EditorComponent: React.FC<EditorComponentProps> = ({
  value,
  onChange,
  commands,
  language,
  onFocus,
  onBlur,
}) => {
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const [theme, setTheme] = useState<string>(
    Utils.getTheme() === "dark" ? "vs-dark" : "vs-light",
  );

  useEffect(() => {
    const handleThemeChange = () => {
      setTheme(Utils.getTheme() === "dark" ? "vs-dark" : "vs-light");
    };

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
      editor.addCommand(
        monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS,
        command.exec,
      );
    });
  }, [commands]);

  const handleEditorDidMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    monacoRef.current = monaco;

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
      onChange={(value) => onChange(value || "")}
      onMount={handleEditorDidMount}
    />
  );
};

export default EditorComponent;

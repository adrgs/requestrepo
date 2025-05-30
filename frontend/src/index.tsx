import React from "react";
import { createRoot } from "react-dom/client";
import App from "./app";
import { HashRouter } from "react-router-dom";

const root = createRoot(document.getElementById("root") as HTMLElement);
root.render(
  <HashRouter>
    <App />
  </HashRouter>,
);

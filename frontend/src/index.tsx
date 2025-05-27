import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app";
import { HashRouter } from "react-router-dom";

const root = ReactDOM.createRoot(
  document.getElementById("root") as HTMLElement,
);
root.render(
  <HashRouter>
    <App></App>
  </HashRouter>,
);

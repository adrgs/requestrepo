import "react-app-polyfill/ie11";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
//import * as serviceWorker from './serviceWorker';
import { HashRouter } from "react-router-dom";
import reportWebVitals from "./reportWebVitals";

const root = ReactDOM.createRoot(document.getElementById("root"));
root.render(
  <HashRouter>
    <App></App>
  </HashRouter>
);

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();

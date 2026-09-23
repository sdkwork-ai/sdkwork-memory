import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App.tsx";
import { bootstrapMemoryPcRuntime } from "./bootstrap/runtime.ts";
import "./index.css";
import "@sdkwork/memory-pc-commons/styles.css";

const rootElement = document.getElementById("root");
if (!rootElement) throw new Error("Application root element is missing");

const root = createRoot(rootElement);
root.render(<div className="bootstrap-state" role="status">SDKWork Memory</div>);

void bootstrapMemoryPcRuntime().then((runtime) => {
  root.render(<StrictMode><App runtime={runtime} /></StrictMode>);
}).catch(() => {
  root.render(<div className="fatal-state" role="alert">Runtime configuration could not be loaded.</div>);
});

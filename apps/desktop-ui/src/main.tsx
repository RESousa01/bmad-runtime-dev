import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@sapphirus/ui/tokens.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";
import "./forge-v2.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Forge could not find its application root.");
}

document.documentElement.dataset.appShell = "forge-v2";

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

import "@fontsource-variable/fraunces/full.css";
import "@fontsource-variable/atkinson-hyperlegible-next/index.css";
import "./styles/tokens.css";
import "./styles/base.css";
import "./ui/ui.css";
import "./app/app.css";

import { QueryClientProvider } from "@tanstack/react-query";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { queryClient } from "./app/query";
import { getLocale } from "./paraglide/runtime.js";

document.documentElement.lang = getLocale();

const root = document.getElementById("root");
if (!root) throw new Error("#root introuvable");

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </StrictMode>,
);

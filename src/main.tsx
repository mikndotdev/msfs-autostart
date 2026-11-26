import { NextStepProvider, NextStepReact } from "nextstepjs";
import { useReactRouterAdapter } from "nextstepjs/adapters/react-router";
import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router";
import App from "./App";
import { AppProvider } from "./state";
import { TourCard, tours } from "./tour";
import "./index.css";

// Hash routing: the production build is served from Tauri's custom protocol, where
// path-based URLs wouldn't resolve on reload.
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <HashRouter>
      <NextStepProvider>
        <NextStepReact
          steps={tours}
          cardComponent={TourCard}
          navigationAdapter={useReactRouterAdapter}
          shadowRgb="0, 0, 0"
          shadowOpacity="0.6"
          // The arrow is drawn with currentColor; match the card background.
          arrowStyle={{ color: "var(--color-base-200)" }}
          disableConsoleLogs
        >
          <AppProvider>
            <App />
          </AppProvider>
        </NextStepReact>
      </NextStepProvider>
    </HashRouter>
  </React.StrictMode>,
);

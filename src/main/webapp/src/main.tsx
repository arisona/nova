import * as React from "react";
import * as ReactDOM from "react-dom/client";

import { ThemeProvider } from "@emotion/react";

import { CssBaseline } from "@mui/material";
import { createTheme } from "@mui/material/styles";

import "@fontsource/roboto/latin-300.css";
import "@fontsource/roboto/latin-400.css";
import "@fontsource/roboto/latin-500.css";
import "@fontsource/roboto/latin-700.css";

import App from "./App";

// A custom theme for this app
const theme = createTheme({
  palette: {
    mode: "dark",
  },
});

const rootElement = document.getElementById("root");
if (rootElement) {
  ReactDOM.createRoot(rootElement).render(
    <React.StrictMode>
      <ThemeProvider theme={theme}>
        <CssBaseline />
        <App />
      </ThemeProvider>
    </React.StrictMode>,
  );
} else {
  console.error("Root element not found");
}

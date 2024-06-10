import { Box, Container } from "@mui/material";
import React from "react";
import { Route, BrowserRouter as Router, Routes } from "react-router-dom";
import { MainPage } from "./MainPage";
import { SettingsPage } from "./SettingsPage";
import { apiGetState } from "./api";

export interface NovaState {
  availableContent: { index: number; name: string }[];
  enabledContent: { index: number; name: string }[];
  selectedContentIndex: number;
  hue: number;
  saturation: number;
  brightness: number;
  speed: number;
  flip: boolean;
  cycleDuration: number;
  ethernetInterface: string;
  module0Address: string;
  statusOk: boolean;
  statusMessage: string;
}

export const defaultNovaState: NovaState = {
  availableContent: [],
  enabledContent: [],
  selectedContentIndex: -1,
  hue: 0.5,
  saturation: 1,
  brightness: 0.5,
  speed: 0.5,
  flip: false,
  cycleDuration: 0,
  ethernetInterface: "eth0",
  module0Address: "1",
  statusOk: false,
  statusMessage: "Unkown error",
};

const pollInterval = 500;

export const App = () => {
  const [state, setState] = React.useState<NovaState>(defaultNovaState);
  const stateRef = React.useRef(state);

  React.useEffect(() => {
    stateRef.current = state;
  }, [state]);

  const handleRefresh = () => {
    apiGetState(stateRef.current).then((newState) => {
      if (newState) {
        setState(newState);
      }
    });
  };

  React.useEffect(() => {
    const intervalId = setInterval(handleRefresh, pollInterval);
    return () => clearInterval(intervalId);
  }, []);

  return (
    <Container maxWidth="sm">
      <Box sx={{ my: 4 }}>
        <Router>
          <Routes>
            <Route
              path="/"
              element={<MainPage state={state} setState={setState} />}
            />
            <Route
              path="/settings"
              element={<SettingsPage state={state} setState={setState} />}
            />
          </Routes>
        </Router>
      </Box>
    </Container>
  );
};

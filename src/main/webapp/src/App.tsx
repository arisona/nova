import { Route, BrowserRouter as Router, Routes } from "react-router-dom";
import { MainPage } from "./MainPage";
import { SettingsPage } from "./SettingsPage";
import React from "react";
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
}

export const defaultNovaState: NovaState = {
  availableContent: [],
  enabledContent: [],
  selectedContentIndex: -1,
  hue: 0.5,
  saturation: 1,
  brightness: 1,
  speed: 0.5,
  flip: false,
  cycleDuration: 0,
  ethernetInterface: "eth0",
  module0Address: "1",
};

export const App = () => {
  const [state, setState] = React.useState<NovaState>(defaultNovaState);

  const handleRefresh = () => {
    apiGetState().then((state) => setState(state));
  };

  React.useEffect(() => handleRefresh(), []);

  return (
    <Router>
      <Routes>
        <Route
          path="/"
          element={
            <MainPage
              state={state}
              setState={setState}
              handleRefresh={handleRefresh}
            />
          }
        />
        <Route
          path="/settings"
          element={
            <SettingsPage
              state={state}
              setState={setState}
              handleRefresh={handleRefresh}
            />
          }
        />
      </Routes>
    </Router>
  );
};

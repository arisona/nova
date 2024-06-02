import { Route, BrowserRouter as Router, Routes } from "react-router-dom";
import { MainPage } from "./MainPage";
import { SettingsPage } from "./SettingsPage";

export interface NovaState {
  availableContent: string[];
  enabledContent: string[];
  selectedContentIndex: string;
  brightness: number;
  hue: number;
  saturation: number;
  speed: number;
  flip: boolean;
  ethernetInterface: string;
  ethernetAddress: string;
}

export const defaultNovaState: NovaState = {
  availableContent: ["No Content"],
  enabledContent: ["No Content"],
  selectedContentIndex: "0",
  brightness: 1,
  hue: 0.5,
  saturation: 1,
  speed: 0.5,
  flip: false,
  ethernetInterface: "eth0",
  ethernetAddress: "1",
};

export function App() {
  return (
    <Router>
      <Routes>
        <Route path="/" element={<MainPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Routes>
    </Router>
  );
}

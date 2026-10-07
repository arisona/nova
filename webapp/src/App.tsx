import { Box, Container } from '@mui/material';
import React from 'react';
import { Route, BrowserRouter as Router, Routes } from 'react-router-dom';
import { CalibrationPage } from './CalibrationPage';
import { MainPage } from './MainPage';
import { SettingsPage } from './SettingsPage';
import { apiGetState, apiGetStatus, onApiRejected } from './api';
import { Status } from './Status';

export interface PaletteColor {
  name: string;
  code: string;
  hex: string;
}

export interface Palette {
  name: string;
  colors: PaletteColor[];
}

/** Per channel: red, green, blue. */
export type Rgb = [number, number, number];

export interface Calibration {
  gamma: Rgb;
  gain: Rgb;
}

export interface NovaState {
  availableContent: string[];
  enabledContent: string[];
  selectedContent: string;
  brightness: number;
  volume: number; // -1 while audio is disabled or failing
  palettes: Palette[];
  palette: string;
  heat: number;
  flow: number;
  form: number;
  void: number;
  flip: boolean;
  calibration: Calibration;
  module0Address: string;
}

export const defaultNovaState: NovaState = {
  availableContent: [],
  enabledContent: [],
  selectedContent: '',
  brightness: 0.5,
  volume: -1,
  palettes: [],
  palette: '',
  heat: 0.5,
  flow: 0.5,
  form: 0.5,
  void: 0.5,
  flip: false,
  calibration: { gamma: [1, 1, 1], gain: [1, 1, 1] },
  module0Address: '1',
};

export interface NovaStatus {
  statusOk: boolean;
  statusMessage: string;
  audioOk: boolean;
  audioMessage: string;
}

export const defaultNovaStatus: NovaStatus = {
  statusOk: false,
  statusMessage: 'Unknown error',
  audioOk: false,
  audioMessage: '',
};

const pollInterval = 500;

export const App = () => {
  const [state, setState] = React.useState(defaultNovaState);
  const [status, setStatus] = React.useState(defaultNovaStatus);

  const loadState = React.useCallback(() => {
    void apiGetState().then((newState) => {
      if (newState) setState(newState);
    });
  }, []);

  React.useEffect(() => {
    // Reload when the page becomes visible again (e.g. switching back on a phone) and
    // whenever the server rejects a change, so renamed or removed content and palettes
    // never linger in the UI.
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') loadState();
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);
    const unsubscribe = onApiRejected(loadState);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      unsubscribe();
    };
  }, [loadState]);

  // Loads the initial state, and reloads it whenever audio becomes ready or fails, so
  // Volume appears or disappears with it.
  React.useEffect(loadState, [loadState, status.audioOk, status.audioMessage]);

  React.useEffect(() => {
    const intervalId = setInterval(handleRefresh, pollInterval);
    return () => {
      clearInterval(intervalId);
    };
  }, []);

  const handleRefresh = () => {
    void apiGetStatus().then((newStatus) => {
      setStatus(newStatus);
    });
  };

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
            <Route
              path="/calibration"
              element={<CalibrationPage state={state} setState={setState} />}
            />
          </Routes>
        </Router>
      </Box>

      <Status ok={status.statusOk} message={status.statusMessage} />
      {!status.audioOk && status.audioMessage && (
        <Status ok={false} message={status.audioMessage} />
      )}
    </Container>
  );
};

import { Box, Container } from '@mui/material';
import React from 'react';
import { Route, BrowserRouter as Router, Routes } from 'react-router-dom';
import { MainPage } from './MainPage';
import { SettingsPage } from './SettingsPage';
import { apiGetState, apiGetStatus } from './api';
import { Status } from './Status';

export interface NovaState {
  availableContent: { index: number; name: string }[];
  enabledContent: { index: number; name: string }[];
  selectedContentIndex: number;
  audioEnabled: boolean;
  brightness: number;
  volume: number;
  tone: number;
  heat: number;
  flow: number;
  form: number;
  flip: boolean;
  ethernetInterface: string;
  module0Address: string;
}

export const defaultNovaState: NovaState = {
  availableContent: [],
  enabledContent: [],
  selectedContentIndex: -1,
  audioEnabled: false,
  brightness: 0.5,
  volume: 0.0,
  tone: 0.0,
  heat: 0.0,
  flow: 0.0,
  form: 0.0,
  flip: false,
  ethernetInterface: 'eth0',
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

  React.useEffect(() => {
    void apiGetState().then((newState) => {
      setState(newState);
    });
  }, []);

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
          </Routes>
        </Router>
      </Box>

      <Status ok={status.statusOk} message={status.statusMessage} />
      {state.audioEnabled && !status.audioOk && status.audioMessage && (
        <Status ok={false} message={status.audioMessage} />
      )}
    </Container>
  );
};

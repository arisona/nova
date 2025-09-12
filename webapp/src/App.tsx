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
  glow: number;
  tone: number;
  punch: number;
  flow: number;
  form: number;
  flip: boolean;
  cycleDuration: string;
  ethernetInterface: string;
  module0Address: string;
}

export const defaultNovaState: NovaState = {
  availableContent: [],
  enabledContent: [],
  selectedContentIndex: -1,
  glow: 0.5,
  tone: 0.0,
  punch: 0.0,
  flow: 0.0,
  form: 0.0,
  flip: false,
  cycleDuration: '0',
  ethernetInterface: 'eth0',
  module0Address: '1',
};

export interface NovaStatus {
  statusOk: boolean;
  statusMessage: string;
}

export const defaultNovaStatus: NovaStatus = {
  statusOk: false,
  statusMessage: 'Unknown error',
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
    </Container>
  );
};

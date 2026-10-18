import { Divider, IconButton, Stack, Tooltip, Typography } from '@mui/material';
import * as React from 'react';

import { NovaState } from './App';
import { ContentSelector } from './ContentSelector';
import { InstrumentSelector } from './InstrumentSelector';
import { PaletteSelector } from './PaletteSelector';
import { Slider } from './Slider';
import { apiSetValue } from './api';
import {
  Acute,
  BlurOn,
  Heat,
  ScatterPlot,
  Settings,
  Sunny,
  VolumeOff,
  VolumeUp,
} from './icons';

import { useNavigate } from 'react-router-dom';
import { useDelayedSend } from './CycleButton';

const sendContent = (name: string) => {
  apiSetValue('selected-content', name);
};

const sendPalette = (name: string) => {
  apiSetValue('palette', name);
};

const sendInstrument = (name: string) => {
  apiSetValue('instrument', name);
};

// Icons: Material Symbols, Rounded and filled (see icons.tsx), https://fonts.google.com/icons

export const MainPage = ({
  state,
  setState,
}: {
  state: NovaState;
  setState: React.Dispatch<React.SetStateAction<NovaState>>;
}) => {
  const navigate = useNavigate();
  const content = useDelayedSend(sendContent);
  const palette = useDelayedSend(sendPalette);
  const instrument = useDelayedSend(sendInstrument);

  const handleSettings = () => {
    void navigate('/settings');
  };

  const paletteColors =
    state.palettes
      .find((palette) => palette.name === state.palette)
      ?.colors.map((color) => color.hex) ?? [];

  const handleContentChange = (name: string) => {
    content.select(name);
    setState((prevState) => ({ ...prevState, selectedContent: name }));
  };

  const handleInstrumentChange = (name: string) => {
    instrument.select(name);
    setState((prevState) => ({ ...prevState, instrument: name }));
  };

  const handleBrightnessChange = (
    _event: Event,
    newValue: number | number[]
  ) => {
    apiSetValue('brightness', newValue as number);
    setState((prevState) => ({
      ...prevState,
      brightness: newValue as number,
    }));
  };

  const handleVolumeChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('volume', newValue as number);
    setState((prevState) => ({ ...prevState, volume: newValue as number }));
  };

  const handlePaletteChange = (name: string) => {
    palette.select(name);
    setState((prevState) => ({ ...prevState, palette: name }));
  };

  const handleHeatChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('heat', newValue as number);
    setState((prevState) => ({
      ...prevState,
      heat: newValue as number,
    }));
  };

  const handleFlowChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('flow', newValue as number);
    setState((prevState) => ({
      ...prevState,
      flow: newValue as number,
    }));
  };

  const handleFormChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('form', newValue as number);
    setState((prevState) => ({
      ...prevState,
      form: newValue as number,
    }));
  };

  const handleVoidChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('void', newValue as number);
    setState((prevState) => ({
      ...prevState,
      void: newValue as number,
    }));
  };

  return (
    <>
      <Stack
        direction="row"
        sx={{ mb: 2, justifyContent: 'space-between', alignItems: 'center' }}
      >
        <Typography variant="h6" align="left">
          NOVA
        </Typography>
        <Stack direction="row" sx={{ alignItems: 'center' }}>
          <Tooltip title="Settings">
            <IconButton aria-label="Settings" onClick={handleSettings}>
              <Settings />
            </IconButton>
          </Tooltip>
        </Stack>
      </Stack>

      <Slider
        icon={<Sunny />}
        label="Brightness"
        value={state.brightness}
        onChange={handleBrightnessChange}
      />
      {state.volume >= 0 && (
        <Slider
          icon={state.volume === 0 ? <VolumeOff /> : <VolumeUp />}
          label="Volume"
          value={state.volume}
          onChange={handleVolumeChange}
        />
      )}
      {state.enabledContent.length > 1 && (
        <ContentSelector
          enabled={state.enabledContent}
          value={state.selectedContent}
          pending={content.pending}
          sent={content.sent}
          colors={paletteColors}
          onChange={handleContentChange}
        />
      )}
      {state.volume >= 0 && state.instruments.length > 1 && (
        <InstrumentSelector
          instruments={state.instruments}
          value={state.instrument}
          pending={instrument.pending}
          sent={instrument.sent}
          colors={paletteColors}
          onChange={handleInstrumentChange}
        />
      )}
      <Divider sx={{ my: 3 }} />
      <PaletteSelector
        palettes={state.palettes}
        value={state.palette}
        pending={palette.pending}
        sent={palette.sent}
        onChange={handlePaletteChange}
      />
      <Slider
        icon={<Heat />}
        label="Heat"
        value={state.heat}
        onChange={handleHeatChange}
      />
      <Slider
        icon={<Acute />}
        label="Flow"
        value={state.flow}
        onChange={handleFlowChange}
      />
      <Slider
        icon={<ScatterPlot />}
        label="Form"
        value={state.form}
        onChange={handleFormChange}
      />
      <Slider
        icon={<BlurOn />}
        label="Void"
        value={state.void}
        onChange={handleVoidChange}
      />
    </>
  );
};

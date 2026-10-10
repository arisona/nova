import {
  BlurOn,
  Contrast,
  Settings,
  Speed,
  WbSunny,
  ScatterPlot,
  VolumeUp,
  VolumeOff,
} from '@mui/icons-material';
import { Divider, IconButton, Stack, Tooltip, Typography } from '@mui/material';
import * as React from 'react';

import { NovaState } from './App';
import { ContentSelector } from './ContentSelector';
import { PaletteSelector } from './PaletteSelector';
import { Slider } from './Slider';
import { apiSetValue } from './api';

import { useNavigate } from 'react-router-dom';

// icons: https://fonts.google.com/icons?icon.set=Material+Icons

export const MainPage = ({
  state,
  setState,
}: {
  state: NovaState;
  setState: React.Dispatch<React.SetStateAction<NovaState>>;
}) => {
  const navigate = useNavigate();

  const handleSettings = () => {
    void navigate('/settings');
  };

  const paletteColors =
    state.palettes
      .find((palette) => palette.name === state.palette)
      ?.colors.map((color) => color.hex) ?? [];

  const handleContentChange = (name: string) => {
    apiSetValue('selected-content', name);
    setState((prevState) => ({ ...prevState, selectedContent: name }));
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
    apiSetValue('palette', name);
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
        icon={<WbSunny />}
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
          colors={paletteColors}
          onChange={handleContentChange}
        />
      )}
      <Divider sx={{ my: 3 }} />
      <PaletteSelector
        palettes={state.palettes}
        value={state.palette}
        onChange={handlePaletteChange}
      />
      <Slider
        icon={<Contrast />}
        label="Heat"
        value={state.heat}
        onChange={handleHeatChange}
      />
      <Slider
        icon={<Speed />}
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

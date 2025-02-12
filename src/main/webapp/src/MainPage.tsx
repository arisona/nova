import {
  Contrast,
  Palette,
  Settings,
  Speed,
  WbSunny,
} from '@mui/icons-material';
import {
  Autocomplete,
  IconButton,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import * as React from 'react';

import { NovaState } from './App';
import { ColorBox } from './ColorBox';
import { Slider } from './Slider';
import { apiSetValue } from './api';
import { hsvToRgb } from './color';

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
    navigate('/settings');
  };

  const handleContentChange = (
    value: { index: number; name: string } | null
  ) => {
    const index = value ? value.index : -1;
    apiSetValue('selected-content-index', index);
    setState((prevState) => ({
      ...prevState,
      selectedContentIndex: index,
    }));
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

  const handleHueChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('hue', newValue as number);
    setState((prevState) => ({ ...prevState, hue: newValue as number }));
  };

  const handleSaturationChange = (
    _event: Event,
    newValue: number | number[]
  ) => {
    apiSetValue('saturation', newValue as number);
    setState((prevState) => ({
      ...prevState,
      saturation: newValue as number,
    }));
  };

  const handleSpeedChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('speed', newValue as number);
    setState((prevState) => ({
      ...prevState,
      speed: newValue as number,
    }));
  };

  const getSelectedContent = () => {
    const selectedContent = state.enabledContent.find(
      (value) => value.index === state.selectedContentIndex
    );
    return selectedContent ? selectedContent : null;
  };

  const rgb = hsvToRgb(state.hue, state.saturation, state.brightness);

  return (
    <>
      <Stack
        direction="row"
        justifyContent="space-between"
        alignItems="center"
        sx={{ mb: 2 }}
      >
        <Typography variant="h6" align="left">
          NOVA
        </Typography>
        <Stack direction="row" alignItems="center">
          <IconButton aris-able="Settings" onClick={handleSettings}>
            <Settings />
          </IconButton>
        </Stack>
      </Stack>

      <Stack direction="row" sx={{ mb: 4 }}>
        <Autocomplete
          fullWidth
          disablePortal
          id="select-content"
          disableCloseOnSelect
          options={state.enabledContent}
          getOptionKey={(option) => option.index}
          getOptionLabel={(option) => option.name}
          renderInput={({ inputProps, ...rest }) => (
            <TextField
              {...rest}
              label="Select content"
              inputProps={{ ...inputProps, readOnly: true }}
            />
          )}
          value={getSelectedContent()}
          onChange={(_event, value) => {
            handleContentChange(value);
          }}
        />
      </Stack>

      <ColorBox r={rgb[0]} g={rgb[1]} b={rgb[2]} />

      <Slider
        icon={<WbSunny />}
        label="Brightness"
        value={state.brightness}
        onChange={handleBrightnessChange}
      />
      <Slider
        icon={<Palette />}
        label="Hue"
        value={state.hue}
        onChange={handleHueChange}
      />
      <Slider
        icon={<Contrast />}
        label="Saturation"
        value={state.saturation}
        onChange={handleSaturationChange}
      />
      <Slider
        icon={<Speed />}
        label="Speed"
        value={state.speed}
        onChange={handleSpeedChange}
      />
    </>
  );
};

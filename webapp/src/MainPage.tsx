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
    void navigate('/settings');
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
    return selectedContent ?? null;
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
          getOptionLabel={(option) => option.name}
          isOptionEqualToValue={(o, v) => o.index === v.index}
          value={getSelectedContent()}
          onChange={(_e, value) => {
            handleContentChange(value);
          }}
          renderInput={(params) => {
            const { InputLabelProps, InputProps, inputProps, ...rest } = params;
            return (
              <TextField
                {...rest}
                label="Select content"
                size="small"
                slotProps={{
                  htmlInput: {
                    ...inputProps,
                    readOnly: true,
                  },
                  inputLabel: InputLabelProps,
                  input: InputProps,
                }}
              />
            );
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

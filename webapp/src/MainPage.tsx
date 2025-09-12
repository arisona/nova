import {
  Contrast,
  Palette,
  Settings,
  Speed,
  WbSunny,
  ScatterPlot,
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
import { ToneChip } from './ToneChip';
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

  const handleGlowChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('glow', newValue as number);
    setState((prevState) => ({
      ...prevState,
      glow: newValue as number,
    }));
  };

  const handleToneChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('tone', newValue as number);
    setState((prevState) => ({ ...prevState, tone: newValue as number }));
  };

  const handlePunchChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('punch', newValue as number);
    setState((prevState) => ({
      ...prevState,
      punch: newValue as number,
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

  const getSelectedContent = () => {
    const selectedContent = state.enabledContent.find(
      (value) => value.index === state.selectedContentIndex
    );
    return selectedContent ?? null;
  };

  const rgb = hsvToRgb(state.tone, state.punch, state.glow);

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

      <Slider
        icon={<WbSunny />}
        label="Glow"
        value={state.glow}
        onChange={handleGlowChange}
      />
      <Slider
        icon={<Palette />}
        label="Tone"
        value={state.tone}
        onChange={handleToneChange}
        endAdornment={<ToneChip r={rgb[0]} g={rgb[1]} b={rgb[2]} />}
      />
      <Slider
        icon={<Contrast />}
        label="Punch"
        value={state.punch}
        onChange={handlePunchChange}
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
    </>
  );
};

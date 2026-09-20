import {
  Contrast,
  Palette,
  Settings,
  Speed,
  WbSunny,
  ScatterPlot,
  VolumeUp,
  VolumeOff,
} from '@mui/icons-material';
import {
  Autocomplete,
  Divider,
  IconButton,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import * as React from 'react';

import { NovaState } from './App';
import { ToneChip } from './ToneChip';
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

  const handleContentChange = (
    value: { index: number; name: string } | null
  ) => {
    if (!value) return;
    const index = value.index;
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

  const handleVolumeChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('volume', newValue as number);
    setState((prevState) => ({ ...prevState, volume: newValue as number }));
  };

  const handleToneChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue('tone', newValue as number);
    setState((prevState) => ({ ...prevState, tone: newValue as number }));
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

  const getSelectedContent = () => {
    const selectedContent = state.enabledContent.find(
      (value) => value.index === state.selectedContentIndex
    );
    return selectedContent ?? null;
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

      {state.enabledContent.length > 1 && (
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
              const { slotProps, ...rest } = params;
              return (
                <TextField
                  {...rest}
                  label="Select content"
                  size="small"
                  slotProps={{
                    ...slotProps,
                    htmlInput: {
                      ...slotProps.htmlInput,
                      readOnly: true,
                    },
                  }}
                />
              );
            }}
          />
        </Stack>
      )}

      <Slider
        icon={<WbSunny />}
        label="Brightness"
        value={state.brightness}
        onChange={handleBrightnessChange}
      />
      {state.audioEnabled && (
        <Slider
          icon={state.volume === 0 ? <VolumeOff /> : <VolumeUp />}
          label="Volume"
          value={state.volume}
          onChange={handleVolumeChange}
        />
      )}
      <Divider sx={{ my: 3 }} />
      <Slider
        icon={<Palette />}
        label="Tone"
        value={state.tone}
        onChange={handleToneChange}
        endAdornment={<ToneChip tone={state.tone} heat={state.heat} />}
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
    </>
  );
};

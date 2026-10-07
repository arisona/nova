import { NavigateBefore } from '@mui/icons-material';
import {
  Button,
  IconButton,
  Slider,
  Stack,
  ToggleButton,
  ToggleButtonGroup,
  Typography,
} from '@mui/material';
import React from 'react';
import { useNavigate } from 'react-router-dom';

import { NovaState, Rgb } from './App';
import { PaletteSelector } from './PaletteSelector';
import { CalibrationPattern, apiGetState, apiSet, apiSetValue } from './api';

// Mirror GAMMA_RANGE and GAIN_RANGE in server/src/calibration.rs.
const GAMMA = { min: 1, max: 3, step: 0.05 };
const GAIN = { min: 0.2, max: 1, step: 0.01 };

const CHANNELS = ['red', 'green', 'blue'] as const;
type Channel = 0 | 1 | 2;

type Pattern = Exclude<CalibrationPattern, 'off'>;
const PATTERNS: { value: Pattern; label: string }[] = [
  { value: 'gray', label: 'Gray' },
  { value: 'red', label: 'Red' },
  { value: 'green', label: 'Green' },
  { value: 'blue', label: 'Blue' },
  { value: 'palette', label: 'Palette' },
];

// The server switches a pattern off 10 minutes after it was last selected, in case this
// page closes without saying so; while the page is visible it keeps selecting it.
const KEEP_ALIVE_MS = 60_000;

const ValueRow = ({
  label,
  value,
  range,
  onChange,
}: {
  label: string;
  value: number;
  range: { min: number; max: number; step: number };
  onChange: (value: number) => void;
}) => {
  return (
    <Stack direction="row" spacing={2} sx={{ alignItems: 'center' }}>
      <Typography variant="body2" sx={{ width: 88, flexShrink: 0 }}>
        {label}
      </Typography>
      <Slider
        aria-label={label}
        value={value}
        min={range.min}
        max={range.max}
        step={range.step}
        onChange={(_event, newValue) => {
          onChange(newValue);
        }}
      />
      <Typography
        variant="body2"
        sx={{
          width: 40,
          flexShrink: 0,
          textAlign: 'right',
          fontVariantNumeric: 'tabular-nums',
        }}
      >
        {value.toFixed(2)}
      </Typography>
    </Stack>
  );
};

export const CalibrationPage = ({
  state,
  setState,
}: {
  state: NovaState;
  setState: React.Dispatch<React.SetStateAction<NovaState>>;
}) => {
  const navigate = useNavigate();
  const [pattern, setPattern] = React.useState<Pattern>('gray');
  const patternRef = React.useRef(pattern);

  // Shows the selected pattern on the display.
  React.useEffect(() => {
    patternRef.current = pattern;
    apiSetValue('calibration-pattern', pattern);
  }, [pattern]);

  // Hides the pattern while the page is hidden, and switches it off when leaving.
  React.useEffect(() => {
    const update = () => {
      apiSetValue(
        'calibration-pattern',
        document.visibilityState === 'visible' ? patternRef.current : 'off'
      );
    };
    document.addEventListener('visibilitychange', update);
    const timer = setInterval(update, KEEP_ALIVE_MS);
    return () => {
      document.removeEventListener('visibilitychange', update);
      clearInterval(timer);
      apiSetValue('calibration-pattern', 'off');
    };
  }, []);

  const setPalette = (name: string) => {
    apiSetValue('palette', name);
    setState((prevState) => ({ ...prevState, palette: name }));
  };

  const setBrightness = (value: number) => {
    apiSetValue('brightness', value);
    setState((prevState) => ({ ...prevState, brightness: value }));
  };

  const setChannel = (
    kind: 'gamma' | 'gain',
    channel: Channel,
    value: number
  ) => {
    apiSetValue(`${kind}-${CHANNELS[channel]}`, value);
    setState((prevState) => {
      const values = [...prevState.calibration[kind]] as Rgb;
      values[channel] = value;
      return {
        ...prevState,
        calibration:
          kind === 'gamma'
            ? { ...prevState.calibration, gamma: values }
            : { ...prevState.calibration, gain: values },
      };
    });
  };

  const setAllGammas = (value: number) => {
    for (const channel of [0, 1, 2] as const)
      setChannel('gamma', channel, value);
  };

  const handleReset = () => {
    void apiSet('reset-calibration')
      .then(() => apiGetState())
      .then((newState) => {
        if (newState) setState(newState);
      });
  };

  const { gamma, gain } = state.calibration;
  const meanGamma = (gamma[0] + gamma[1] + gamma[2]) / 3;

  return (
    <>
      <Stack
        direction="row"
        sx={{ mb: 2, justifyContent: 'space-between', alignItems: 'center' }}
      >
        <Typography variant="h6" align="left">
          CALIBRATION
        </Typography>
        <IconButton
          aria-label="Back"
          onClick={() => {
            void navigate('/settings');
          }}
        >
          <NavigateBefore />
        </IconButton>
      </Stack>

      <Typography variant="body2" sx={{ mb: 3, color: 'text.secondary' }}>
        The display shows a test pattern while this page is open. Adjust the
        gamma until the ten steps from bottom to top look evenly spaced, then
        the white balance until gray looks neutral. The simulator previews the
        calibration only while this page is open.
      </Typography>

      <ToggleButtonGroup
        exclusive
        fullWidth
        size="small"
        value={pattern}
        onChange={(_event, value: Pattern | null) => {
          if (value) setPattern(value);
        }}
        sx={{ mb: 3 }}
      >
        {PATTERNS.map(({ value, label }) => (
          <ToggleButton key={value} value={value}>
            {label}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>

      {pattern === 'palette' && (
        <PaletteSelector
          palettes={state.palettes}
          value={state.palette}
          onChange={setPalette}
        />
      )}

      <Stack spacing={1} sx={{ mb: 3 }}>
        <ValueRow
          label="Brightness"
          value={state.brightness}
          range={{ min: 0, max: 1, step: 0.01 }}
          onChange={setBrightness}
        />
      </Stack>

      <Typography variant="overline" sx={{ color: 'text.secondary' }}>
        Gamma
      </Typography>
      <Stack spacing={1} sx={{ mb: 3 }}>
        <ValueRow
          label="All"
          value={meanGamma}
          range={GAMMA}
          onChange={setAllGammas}
        />
        {CHANNELS.map((name, channel) => (
          <ValueRow
            key={name}
            label={name[0].toUpperCase() + name.slice(1)}
            value={gamma[channel]}
            range={GAMMA}
            onChange={(value) => {
              setChannel('gamma', channel as Channel, value);
            }}
          />
        ))}
      </Stack>

      <Typography variant="overline" sx={{ color: 'text.secondary' }}>
        White balance
      </Typography>
      <Stack spacing={1} sx={{ mb: 4 }}>
        {CHANNELS.map((name, channel) => (
          <ValueRow
            key={name}
            label={name[0].toUpperCase() + name.slice(1)}
            value={gain[channel]}
            range={GAIN}
            onChange={(value) => {
              setChannel('gain', channel as Channel, value);
            }}
          />
        ))}
      </Stack>

      <Button fullWidth variant="outlined" onClick={handleReset}>
        Reset calibration
      </Button>
    </>
  );
};

import { Box, Slider as MuiSlider, Stack, Typography } from '@mui/material';
import * as React from 'react';

interface SliderProps {
  icon: React.ReactNode;
  label: string;
  min?: number;
  max?: number;
  step?: number; // Add this line
  value?: number;
  onChange: (
    event: Event,
    value: number | number[],
    activeThumb: number
  ) => void;
  endAdornment?: React.ReactNode;
}

export const Slider = ({
  icon,
  label,
  min = 0,
  max = 1,
  step = 0.01,
  value = 0,
  onChange,
  endAdornment,
}: SliderProps) => {
  return (
    <Stack spacing={1.5} direction="row" sx={{ mb: 2, alignItems: 'center' }}>
      <Box aria-hidden sx={{ display: 'flex', width: 24, flexShrink: 0 }}>
        {icon}
      </Box>
      <Typography variant="body2" sx={{ width: 72, flexShrink: 0 }}>
        {label}
      </Typography>
      <MuiSlider
        aria-label={label}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={onChange}
        sx={{ flexGrow: 1, minWidth: 0 }}
      />
      <Box sx={{ width: 28, flexShrink: 0 }}>{endAdornment}</Box>
    </Stack>
  );
};

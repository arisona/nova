import { Slider as MuiSlider } from '@mui/material';
import * as React from 'react';

import { ControlRow } from './ControlRow';

interface SliderProps {
  icon: React.ReactNode;
  label: string;
  min?: number;
  max?: number;
  step?: number;
  value?: number;
  onChange: (
    event: Event,
    value: number | number[],
    activeThumb: number
  ) => void;
}

export const Slider = ({
  icon,
  label,
  min = 0,
  max = 1,
  step = 0.01,
  value = 0,
  onChange,
}: SliderProps) => {
  return (
    <ControlRow icon={icon} label={label}>
      <MuiSlider
        aria-label={label}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={onChange}
      />
    </ControlRow>
  );
};

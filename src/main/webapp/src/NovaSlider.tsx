import { Slider, Stack } from "@mui/material";
import * as React from "react";

interface NovaSliderProps {
  icon: React.ReactNode;
  label: string;
  min?: number;
  max?: number;
  step?: number; // Add this line
  value?: number;
  onChange: (
    event: Event,
    value: number | number[],
    activeThumb: number,
  ) => void;
}

export const NovaSlider = ({
  icon,
  label,
  min = 0,
  max = 1,
  step = 0.01,
  value = 0,
  onChange,
}: NovaSliderProps) => {
  return (
    <Stack spacing={2} direction="row" sx={{ mb: 2 }} alignItems="center">
      {icon}
      <Slider
        aria-label={label}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={onChange}
      />
    </Stack>
  );
};

import { Slider as MuiSlider, Stack } from "@mui/material";
import * as React from "react";

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
    activeThumb: number,
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
    <Stack spacing={2} direction="row" alignItems="center" sx={{ mb: 2 }}>
      {icon}
      <MuiSlider
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

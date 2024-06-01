import { Slider, Stack } from "@mui/material";
import * as React from "react";

type NovaSliderProps = {
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
};

export default function NovaSlider({
  icon,
  label,
  min = 0, // default value for min
  max = 1, // default value for max
  step = 0.01, // default value for step
  value = 0, // default value for value
  onChange,
}: NovaSliderProps) {
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
}

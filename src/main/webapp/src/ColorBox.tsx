import { Box } from '@mui/material';
import { rgbToHex } from './color';

interface ColorBoxProps {
  r: number;
  g: number;
  b: number;
}

export const ColorBox = ({ r, g, b }: ColorBoxProps) => {
  const color = rgbToHex(r, g, b);
  const border = rgbToHex(clamp(r * 2), clamp(g * 2), clamp(b * 2));
  return (
    <Box
      sx={{
        mb: 4,
        width: '100%',
        height: 40,
        bgcolor: color,
        border: `1px solid ${border}`,
      }}
    />
  );
};

const clamp = (value: number): number => {
  return Math.min(1, Math.max(0, value));
};

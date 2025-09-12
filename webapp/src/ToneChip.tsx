import { Box } from '@mui/material';
import { rgbToHex } from './color';

export const ToneChip = ({ r, g, b }: { r: number; g: number; b: number }) => {
  const color = rgbToHex(r, g, b);
  const border = rgbToHex(
    Math.min(1, r * 1.8),
    Math.min(1, g * 1.8),
    Math.min(1, b * 1.8)
  );
  return (
    <Box
      aria-label="Tone preview"
      sx={{
        width: 28,
        height: 18,
        borderRadius: 1,
        border: `1px solid ${border}`,
        bgcolor: color,
        flexShrink: 0,
      }}
    />
  );
};

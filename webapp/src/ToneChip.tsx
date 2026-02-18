import { Box } from '@mui/material';

// Minimal chip that previews Tone using CSS OKLCH color space for
// perceptual uniformity. Expects `tone` in [0, 1].
export const ToneChip = ({ tone, heat }: { tone: number; heat: number }) => {
  const L = 0.68;
  const colorfulness = 1 - (1 - Math.max(0, Math.min(1, heat * 2))) ** 3;
  const C = 0.025 + 0.18 * colorfulness;
  const H = Math.max(0, Math.min(1, tone)) * 360;
  // Use fixed-point strings to appease eslint's restrict-template-expressions
  const bg = `oklch(${L.toFixed(2)} ${C.toFixed(4)} ${H.toFixed(1)}deg)`;
  return (
    <Box
      aria-label="Tone preview"
      sx={{
        width: 28,
        height: 18,
        borderRadius: 1,
        // neutral border to remain visible across hues
        border: '1px solid rgba(255, 255, 255, 0.35)',
        bgcolor: bg,
        flexShrink: 0,
      }}
    />
  );
};

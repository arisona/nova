import { Box } from '@mui/material';

// Minimal chip that previews Tone using CSS OKLCH color space for
// perceptual uniformity. Expects `tone` in [0, 1].
export const ToneChip = ({ tone }: { tone: number }) => {
  const L = 0.72; // pleasant lightness for preview
  const C = 0.2; // moderate chroma for even saturation across hues
  const H = Math.max(0, Math.min(1, tone)) * 360;
  // Use fixed-point strings to appease eslint's restrict-template-expressions
  const bg = `oklch(${L.toFixed(2)} ${C.toFixed(2)} ${H.toFixed(1)}deg)`;
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

import { Box } from "@mui/material";
import { rgbToHex } from "./color";

interface NovaColorProps {
  r: number;
  g: number;
  b: number;
}

export default function NovaColor({ r, g, b }: NovaColorProps) {
  const color = rgbToHex(r, g, b);
  const border = rgbToHex(clamp(r * 2), clamp(g * 2), clamp(b * 2));
  return (
    <>
      <Box
        sx={{
          mb: 4,
          width: "100%",
          height: 40,
          bgcolor: color,
          border: `1px solid ${border}`,
        }}
      />
    </>
  );
}

function clamp(value: number): number {
  return Math.min(1, Math.max(0, value));
}

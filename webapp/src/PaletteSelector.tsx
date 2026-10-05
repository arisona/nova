import { keyframes } from '@emotion/react';
import { Palette as PaletteIcon } from '@mui/icons-material';
import { Box, ButtonBase, Typography } from '@mui/material';

import { Palette } from './App';
import { ControlRow } from './ControlRow';

// The palette name shows for about 2 s whenever the palette changes (including the
// first load), then fades out so the strip shows only colors.
const nameFade = keyframes`
  0%, 70% { opacity: 1; }
  100% { opacity: 0; }
`;

// The selected palette as a strip of its colors, as wide as the sliders, with the
// palette's name briefly on top. Tapping the strip selects the next palette, wrapping
// around.
export const PaletteSelector = ({
  palettes,
  value,
  onChange,
}: {
  palettes: Palette[];
  value: string;
  onChange: (name: string) => void;
}) => {
  const index = Math.max(
    0,
    palettes.findIndex((palette) => palette.name === value)
  );
  const palette = palettes.at(index);
  const name = palette?.name ?? 'No palettes';

  const selectNext = () => {
    const next = palettes.at((index + 1) % palettes.length);
    if (next) onChange(next.name);
  };

  return (
    <ControlRow icon={<PaletteIcon />} label="Palette">
      <ButtonBase
        aria-label={`Palette: ${name}. Select for the next palette.`}
        disabled={palettes.length < 2}
        onClick={selectNext}
        sx={{
          position: 'relative',
          display: 'flex',
          width: '100%',
          height: 32,
          borderRadius: 1,
          overflow: 'hidden',
          // neutral border so dark colors stay visible on the dark background
          border: '1px solid rgba(255, 255, 255, 0.25)',
        }}
      >
        {palette?.colors.map((color, position) => (
          <Box
            key={`${String(position)}-${color.hex}`}
            title={`${color.name} ${color.code}`}
            sx={{ flex: 1, alignSelf: 'stretch', bgcolor: color.hex }}
          />
        ))}
        <Typography
          // a new key restarts the fade whenever the palette changes
          key={name}
          variant="body2"
          noWrap
          sx={{
            position: 'absolute',
            top: '50%',
            left: '50%',
            transform: 'translate(-50%, -50%)',
            maxWidth: 'calc(100% - 16px)',
            color: '#fff',
            // a soft dark halo keeps the name readable on light and dark colors
            textShadow:
              '0 0 2px rgba(0, 0, 0, 0.9), 0 0 6px rgba(0, 0, 0, 0.7), 0 0 12px rgba(0, 0, 0, 0.5)',
            pointerEvents: 'none',
            animation: `${nameFade.toString()} 2.6s ease-in forwards`,
            // without motion, the name disappears at once instead of fading
            '@media (prefers-reduced-motion: reduce)': {
              animationTimingFunction: 'step-end',
            },
          }}
        >
          {name}
        </Typography>
      </ButtonBase>
    </ControlRow>
  );
};

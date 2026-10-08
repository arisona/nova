import { Palette as PaletteIcon } from '@mui/icons-material';
import { Box } from '@mui/material';

import { Palette } from './App';
import { ControlRow } from './ControlRow';
import { CycleButton } from './CycleButton';

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
      <CycleButton
        name={name}
        ariaLabel={`Palette: ${name}. Select for the next palette.`}
        disabled={palettes.length < 2}
        onClick={selectNext}
      >
        {palette?.colors.map((color, position) => (
          <Box
            key={`${String(position)}-${color.hex}`}
            title={`${color.name} ${color.code}`}
            sx={{ flex: 1, alignSelf: 'stretch', bgcolor: color.hex }}
          />
        ))}
      </CycleButton>
    </ControlRow>
  );
};

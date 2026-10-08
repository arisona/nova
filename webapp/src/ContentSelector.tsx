import { Animation } from '@mui/icons-material';

import { ControlRow } from './ControlRow';
import { ContentPictogram } from './ContentPictogram';
import { CycleButton } from './CycleButton';

// The selected content module as a pictogram in the palette's colors, as wide as the
// sliders, with the module's name briefly on top. Tapping it selects the next enabled
// module, wrapping around.
export const ContentSelector = ({
  enabled,
  value,
  colors,
  heat,
  onChange,
}: {
  enabled: string[];
  value: string;
  colors: string[];
  heat: number;
  onChange: (name: string) => void;
}) => {
  const index = Math.max(0, enabled.indexOf(value));
  const name = enabled.at(index) ?? value;

  const selectNext = () => {
    const next = enabled.at((index + 1) % enabled.length);
    if (next) onChange(next);
  };

  return (
    <ControlRow icon={<Animation />} label="Content">
      <CycleButton
        name={name}
        ariaLabel={`Content: ${name}. Select for the next content.`}
        disabled={enabled.length < 2}
        onClick={selectNext}
      >
        <ContentPictogram name={name} colors={colors} heat={heat} />
      </CycleButton>
    </ControlRow>
  );
};

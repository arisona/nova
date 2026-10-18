import { Piano } from './icons';

import { ControlRow } from './ControlRow';
import { CycleButton } from './CycleButton';
import { InstrumentSignature } from './InstrumentSignature';

// The selected instrument as its oscillogram in the palette's colors, as wide as the
// sliders, with the instrument's name briefly on top. Tapping it selects the next
// instrument, wrapping around.
export const InstrumentSelector = ({
  instruments,
  value,
  pending,
  sent,
  colors,
  onChange,
}: {
  instruments: string[];
  value: string;
  pending: boolean;
  sent: number;
  colors: string[];
  onChange: (name: string) => void;
}) => {
  const index = Math.max(0, instruments.indexOf(value));
  const name = instruments.at(index) ?? value;

  const selectNext = () => {
    const next = instruments.at((index + 1) % instruments.length);
    if (next) onChange(next);
  };

  return (
    <ControlRow icon={<Piano />} label="Instrument">
      <CycleButton
        name={name}
        pending={pending}
        sent={sent}
        ariaLabel={`Instrument: ${name}. Select for the next instrument.`}
        disabled={instruments.length < 2}
        onClick={selectNext}
      >
        <InstrumentSignature name={name} colors={colors} />
      </CycleButton>
    </ControlRow>
  );
};

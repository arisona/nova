import { keyframes } from '@emotion/react';
import { Box, ButtonBase, Typography } from '@mui/material';
import * as React from 'react';

// The name shows for about 2 s whenever the selection changes (including the first
// load), then fades out so the strip shows only its picture.
const nameFade = keyframes`
  0%, 70% { opacity: 1; }
  100% { opacity: 0; }
`;

// A circle that spreads from the center and fades, like a tap's ripple.
const sentRipple = keyframes`
  from { transform: translate(-50%, -50%) scale(0); opacity: 0.4; }
  to { transform: translate(-50%, -50%) scale(1); opacity: 0; }
`;

const sendDelay = 1000;

// Sends a selection only once it has rested for a second, so cycling quickly through
// choices leaves the display alone until the user stops. `send` must be stable (defined
// outside the component). Leaving the page sends a pending selection at once.
export const useDelayedSend = (send: (name: string) => void) => {
  const [pending, setPending] = React.useState(false);
  const [sent, setSent] = React.useState(0);
  const timer = React.useRef<number | undefined>(undefined);
  const latest = React.useRef('');

  React.useEffect(
    () => () => {
      if (timer.current !== undefined) {
        clearTimeout(timer.current);
        send(latest.current);
      }
    },
    [send]
  );

  const select = (name: string) => {
    latest.current = name;
    setPending(true);
    clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      timer.current = undefined;
      setPending(false);
      setSent((count) => count + 1);
      send(name);
    }, sendDelay);
  };

  return { pending, sent, select };
};

// A strip as wide as the sliders that shows the current selection, with its name
// briefly on top. Tapping it selects the next one; the caller wraps around. While a
// selection is `pending` (not yet sent), its name stays; once sent (`sent` counts the
// sends), the strip ripples and the name fades. Taps themselves do not ripple.
export const CycleButton = ({
  name,
  pending,
  sent,
  ariaLabel,
  disabled,
  onClick,
  children,
}: {
  name: string;
  pending: boolean;
  sent: number;
  ariaLabel: string;
  disabled: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) => {
  return (
    <ButtonBase
      aria-label={ariaLabel}
      disabled={disabled}
      onClick={onClick}
      // taps do not ripple; the ripple below shows when a choice is sent
      disableRipple
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
      {children}
      {sent > 0 && (
        <Box
          // a new key restarts the ripple with every send
          key={sent}
          sx={{
            position: 'absolute',
            top: '50%',
            left: '50%',
            width: '100%',
            aspectRatio: '1',
            borderRadius: '50%',
            bgcolor: '#fff',
            opacity: 0,
            pointerEvents: 'none',
            animation: `${sentRipple.toString()} 0.6s ease-out`,
            '@media (prefers-reduced-motion: reduce)': { animation: 'none' },
          }}
        />
      )}
      <Typography
        // a new key restarts the fade whenever the selection changes or is sent
        key={`${name}-${String(pending)}`}
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
          animation: pending
            ? 'none'
            : `${nameFade.toString()} 2.6s ease-in forwards`,
          // without motion, the name disappears at once instead of fading
          '@media (prefers-reduced-motion: reduce)': {
            animationTimingFunction: 'step-end',
          },
        }}
      >
        {name}
      </Typography>
    </ButtonBase>
  );
};

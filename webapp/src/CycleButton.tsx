import { keyframes } from '@emotion/react';
import { ButtonBase, Typography } from '@mui/material';
import * as React from 'react';

// The name shows for about 2 s whenever the selection changes (including the first
// load), then fades out so the strip shows only its picture.
const nameFade = keyframes`
  0%, 70% { opacity: 1; }
  100% { opacity: 0; }
`;

// A strip as wide as the sliders that shows the current selection, with its name
// briefly on top. Tapping it selects the next one; the caller wraps around.
export const CycleButton = ({
  name,
  ariaLabel,
  disabled,
  onClick,
  children,
}: {
  name: string;
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
      <Typography
        // a new key restarts the fade whenever the selection changes
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
  );
};

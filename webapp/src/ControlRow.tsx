import { Box, Stack, Typography } from '@mui/material';
import * as React from 'react';

// Show text labels next to the control icons. Off: the icons are enough, and users find
// out what each control does by exploring. Labels remain available to screen readers.
export const SHOW_LABELS = false as boolean;

// One control: icon, optional label, and the control itself, which takes the remaining
// width so every control in the list lines up.
export const ControlRow = ({
  icon,
  label,
  children,
}: {
  icon: React.ReactNode;
  label: string;
  children: React.ReactNode;
}) => {
  return (
    <Stack spacing={1.5} direction="row" sx={{ mb: 2, alignItems: 'center' }}>
      <Box aria-hidden sx={{ display: 'flex', width: 24, flexShrink: 0 }}>
        {icon}
      </Box>
      {SHOW_LABELS && (
        <Typography variant="body2" sx={{ width: 72, flexShrink: 0 }}>
          {label}
        </Typography>
      )}
      <Box sx={{ display: 'flex', flexGrow: 1, minWidth: 0 }}>{children}</Box>
    </Stack>
  );
};

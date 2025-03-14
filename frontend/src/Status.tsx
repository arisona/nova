import { CheckCircle, OfflineBolt } from '@mui/icons-material';
import { Box, Stack, Typography } from '@mui/material';

interface StatusProps {
  ok: boolean;
  message: string;
}

export const Status = ({ ok, message }: StatusProps) => {
  return (
    <>
      <Box
        sx={{
          border: '1px dashed',
          borderColor: 'divider',
          mt: 3,
          mb: 3,
        }}
      />
      <Stack
        spacing={2}
        direction="row"
        alignItems="center"
        sx={{ mt: 2, mb: 2 }}
      >
        {ok ? <CheckCircle /> : <OfflineBolt />}
        <Typography variant="body2" align="left">
          {message}
        </Typography>
      </Stack>{' '}
    </>
  );
};

import { NavigateBefore } from '@mui/icons-material';
import {
  Box,
  Button,
  Checkbox,
  FormControlLabel,
  FormGroup,
  IconButton,
  InputLabel,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Stack,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import React from 'react';
import { useNavigate } from 'react-router-dom';
import { NovaState } from './App';
import { apiGetState, apiSet, apiSetValue } from './api';

export const SettingsPage = ({
  state,
  setState,
}: {
  state: NovaState;
  setState: React.Dispatch<React.SetStateAction<NovaState>>;
}) => {
  const navigate = useNavigate();

  const handleBack = () => {
    void navigate('/');
  };

  const isLastEnabled = (name: string) =>
    state.enabledContent.length === 1 && state.enabledContent[0] === name;

  const handleEnabledContentChange = (name: string) => {
    // At least one module stays enabled; the server rejects an empty list.
    if (isLastEnabled(name)) return;
    const enabled = state.enabledContent.includes(name);
    const enabledContent = state.availableContent.filter((item) =>
      item === name ? !enabled : state.enabledContent.includes(item)
    );
    apiSetValue('enabled-content', enabledContent.join(','));
    setState((prevState) => ({
      ...prevState,
      enabledContent,
      selectedContent: enabledContent.includes(prevState.selectedContent)
        ? prevState.selectedContent
        : (enabledContent[0] ?? ''),
    }));
  };

  const handleFlipChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const flip = event.target.checked;
    apiSetValue('flip-vertical', flip);
    setState((prevState) => ({ ...prevState, flip: flip }));
  };

  const [ethernetInterfaceInputState, setEthernetInterfaceInputState] =
    React.useState<string>('');

  const handleEthernetInterfaceChange = (
    event: React.ChangeEvent<HTMLInputElement>
  ) => {
    const eif = event.target.value;
    if (eif === '') {
      setEthernetInterfaceInputState(
        'Enter a valid interface name (e.g. eth0)'
      );
    } else {
      setEthernetInterfaceInputState('');
      apiSetValue('ethernet-interface', eif);
    }
    setState((prevState) => ({ ...prevState, ethernetInterface: eif }));
  };

  const [ethernetAddressInputState, setEthernetAddressInputState] =
    React.useState<string>('');

  const handleEthernetAddressChange = (
    event: React.ChangeEvent<HTMLInputElement>
  ) => {
    const eaddr = event.target.value;
    if (+state.module0Address === -1) {
      setEthernetAddressInputState('Not configurable');
      return;
    }

    if (eaddr === '' || isNaN(+eaddr) || +eaddr < 1 || +eaddr > 255) {
      setEthernetAddressInputState('Enter a valid module address (e.g. 1)');
    } else {
      setEthernetAddressInputState('');
      apiSetValue('module0-address', +eaddr);
    }
    if (+eaddr !== -1)
      setState((prevState) => ({ ...prevState, module0Address: eaddr }));
  };

  const handleRestore = () => {
    void apiSet('restore')
      .then((response) => {
        if (!response.ok) throw new Error(String(response.status));
        return apiGetState();
      })
      .then((restoredState) => {
        if (restoredState) setState(restoredState);
        setEthernetInterfaceInputState('');
        setEthernetAddressInputState('');
      })
      .catch((error: unknown) => {
        console.error('Restore defaults failed:', error);
      });
  };

  const handleReset = () => {
    void apiSet('reset');
  };

  const handleReload = () => {
    void apiSet('reload');
  };

  return (
    <>
      <Stack
        direction="row"
        sx={{ mb: 2, justifyContent: 'space-between', alignItems: 'center' }}
      >
        <Typography variant="h6" align="left">
          SETTINGS
        </Typography>
        <Stack direction="row">
          <IconButton aria-label="Back" onClick={handleBack}>
            <NavigateBefore />
          </IconButton>
        </Stack>
      </Stack>

      <InputLabel id="select-enabled-content-label" sx={{ mb: 1 }}>
        Select enabled content
      </InputLabel>
      <Box
        sx={{
          border: '1px solid',
          borderColor: 'divider',
          borderRadius: 1,
          px: 1,
          py: 0,
          minHeight: '16em',
          maxHeight: '16em',
          overflow: 'auto',
          mb: 3,
        }}
      >
        <List dense>
          {state.availableContent.map((name) => (
            <ListItem key={name} disablePadding>
              <ListItemButton
                dense
                disableRipple
                disabled={isLastEnabled(name)}
                onClick={() => {
                  handleEnabledContentChange(name);
                }}
              >
                <ListItemIcon>
                  <Checkbox
                    disableRipple
                    checked={state.enabledContent.includes(name)}
                  />
                </ListItemIcon>
                <ListItemText primary={name} />
              </ListItemButton>
            </ListItem>
          ))}
        </List>{' '}
      </Box>

      <FormGroup sx={{ pt: 1, mb: 8 }}>
        <FormControlLabel
          control={<Switch checked={state.flip} onChange={handleFlipChange} />}
          label="Flip content vertically"
        />
      </FormGroup>

      <InputLabel id="network-settings-label" sx={{ mb: 2 }}>
        Ethernet settings (reload server to apply changes)
      </InputLabel>
      <Stack spacing={2} direction="row" sx={{ mb: 8 }}>
        <TextField
          fullWidth
          label="Ethernet interface"
          value={state.ethernetInterface}
          onChange={handleEthernetInterfaceChange}
          error={ethernetInterfaceInputState !== ''}
          helperText={ethernetInterfaceInputState}
        />
        <TextField
          fullWidth
          label="Module address"
          value={
            +state.module0Address === -1
              ? 'Not configurable'
              : state.module0Address
          }
          onChange={handleEthernetAddressChange}
          disabled={+state.module0Address === -1}
          error={ethernetAddressInputState !== ''}
          helperText={ethernetAddressInputState}
        />
      </Stack>

      <Stack spacing={2} direction="row" sx={{ mb: 4 }}>
        <Button fullWidth variant="outlined" onClick={handleRestore}>
          Restore defaults
        </Button>
        <Button fullWidth variant="outlined" onClick={handleReset}>
          Reset hardware
        </Button>
        <Button fullWidth variant="outlined" onClick={handleReload}>
          Reload server
        </Button>
      </Stack>
    </>
  );
};

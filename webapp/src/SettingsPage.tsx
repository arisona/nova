import { NavigateBefore } from '@mui/icons-material';
import {
  Box,
  Button,
  Checkbox,
  FormControlLabel,
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

  const [moduleAddressInputState, setModuleAddressInputState] =
    React.useState<string>('');

  const handleModuleAddressChange = (
    event: React.ChangeEvent<HTMLInputElement>
  ) => {
    const address = event.target.value;
    if (+state.module0Address === -1) {
      setModuleAddressInputState('Not configurable');
      return;
    }

    if (address === '' || isNaN(+address) || +address < 1 || +address > 255) {
      setModuleAddressInputState('Enter a valid module address (e.g. 1)');
    } else {
      setModuleAddressInputState('');
      apiSetValue('module0-address', +address);
    }
    if (+address !== -1)
      setState((prevState) => ({ ...prevState, module0Address: address }));
  };

  const handleRestore = () => {
    void apiSet('restore')
      .then((response) => {
        if (!response.ok) throw new Error(String(response.status));
        return apiGetState();
      })
      .then((restoredState) => {
        if (restoredState) setState(restoredState);
        setModuleAddressInputState('');
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

      <Stack
        spacing={2}
        direction="row"
        sx={{ pt: 1, mb: 8, alignItems: 'flex-start' }}
      >
        <TextField
          sx={{ flex: 1 }}
          label="Module 0 address"
          value={
            +state.module0Address === -1
              ? 'Not configurable'
              : state.module0Address
          }
          onChange={handleModuleAddressChange}
          disabled={+state.module0Address === -1}
          error={moduleAddressInputState !== ''}
          helperText={moduleAddressInputState}
        />
        <FormControlLabel
          // The height of the text field's input, so the switch centres on it.
          sx={{ flex: 1, height: 56 }}
          control={<Switch checked={state.flip} onChange={handleFlipChange} />}
          label="Flip content vertically"
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

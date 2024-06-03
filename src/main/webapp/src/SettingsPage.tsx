import {
  CheckBox,
  CheckBoxOutlineBlank,
  NavigateBefore,
} from "@mui/icons-material";
import {
  Autocomplete,
  Box,
  Button,
  Checkbox,
  Container,
  FormControlLabel,
  FormGroup,
  IconButton,
  Stack,
  Switch,
  TextField,
  Typography,
} from "@mui/material";
import React from "react";
import { useNavigate } from "react-router-dom";
import { NovaState, defaultNovaState } from "./App";
import { apiGetState, apiSet, apiSetValue } from "./api";

export function SettingsPage() {
  const navigate = useNavigate();

  const [state, setState] = React.useState<NovaState>(defaultNovaState);

  const handleBack = () => {
    navigate("/");
  };

  const handleEnabledContentChange = (
    values: {
      index: number;
      name: string;
    }[],
  ) => {
    const indices = values
      .map((value) => value.index)
      .sort((a, b) => a - b)
      .join(" ");
    apiSetValue("enabled-content-indices", indices);
    setState((prevState) => ({ ...prevState, enabledContent: values }));
  };

  const handleFlipChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const flip = event.target.checked;
    apiSetValue("flip", flip);
    setState((prevState) => ({ ...prevState, flip: flip }));
  };

  const handleEthernetInterfaceChange = (
    event: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const eif = event.target.value;
    apiSetValue("ethernet-interface", eif);
    setState((prevState) => ({ ...prevState, ethernetInterface: eif }));
  };

  const handleEthernetAddressChange = (
    event: React.ChangeEvent<HTMLInputElement>,
  ) => {
    const eaddr = event.target.value;
    apiSetValue("ethernet-address", eaddr);
    setState((prevState) => ({ ...prevState, ethernetAddress: eaddr }));
  };

  const handleRestore = () => {
    apiSet("restore");
    handleRefresh();
  };

  const handleReset = () => {
    apiSet("reset");
  };

  const handleReload = () => {
    apiSet("reload");
  };

  const handleRefresh = () => {
    apiGetState().then((state) => setState(state));
  };

  const getEnabledContent = () => {
    if (state.enabledContent.length) {
      return state.enabledContent;
    } else {
      return [];
    }
  };

  React.useEffect(() => handleRefresh(), []);

  return (
    <Container maxWidth="sm">
      <Box sx={{ my: 4 }}>
        <Stack
          direction="row"
          justifyContent="space-between"
          alignItems="center"
          sx={{ mb: 2 }}
        >
          <Typography variant="h3" component="h3" align="left">
            SETTINGS
          </Typography>
          <Stack direction="row">
            <IconButton aria-label="Back" onClick={handleBack}>
              <NavigateBefore />
            </IconButton>
          </Stack>
        </Stack>

        <Stack direction="row" sx={{ mb: 4 }}>
          <Autocomplete
            fullWidth
            disablePortal
            id="select-enabled-content"
            multiple
            disableCloseOnSelect
            options={state.availableContent}
            getOptionKey={(option) => option.index}
            getOptionLabel={(option) => option.name}
            renderTags={() => null}
            renderInput={({ inputProps, ...rest }) => (
              <TextField
                {...rest}
                label="Select Enabled Content"
                inputProps={{ ...inputProps, readOnly: true }}
              />
            )}
            renderOption={(props, option, { selected }) => (
              <li {...props} key={option.index}>
                <Checkbox
                  icon={<CheckBoxOutlineBlank fontSize="small" />}
                  checkedIcon={<CheckBox fontSize="small" />}
                  style={{ marginRight: 8 }}
                  checked={selected}
                />
                {option.name}
              </li>
            )}
            value={getEnabledContent()}
            isOptionEqualToValue={(option, value) =>
              option.index === value.index
            }
            onChange={(_event, value) => {
              handleEnabledContentChange(value);
            }}
          />
        </Stack>

        <Stack spacing={2} direction="row" sx={{ mb: 4 }}>
          <FormGroup>
            <FormControlLabel
              control={
                <Switch checked={state.flip} onChange={handleFlipChange} />
              }
              label="Flip Content Vertically"
            />
          </FormGroup>
        </Stack>

        <Stack spacing={2} direction="row" sx={{ mb: 8 }}>
          <TextField
            fullWidth
            label="Ethernet Interface"
            value={state.ethernetInterface}
            onChange={handleEthernetInterfaceChange}
          />
          <TextField
            fullWidth
            label="Module Ethernet Address"
            value={state.ethernetAddress}
            onChange={handleEthernetAddressChange}
          />
        </Stack>

        <Stack spacing={2} direction="row" sx={{ mb: 4 }}>
          <Button fullWidth variant="outlined" onClick={handleRestore}>
            Restore Defaults
          </Button>
          <Button fullWidth variant="outlined" onClick={handleReset}>
            Reset Hardware
          </Button>
          <Button fullWidth variant="outlined" onClick={handleReload}>
            Reload Server
          </Button>
        </Stack>
      </Box>
    </Container>
  );
}

import { NavigateBefore } from "@mui/icons-material";
import {
  Box,
  Button,
  Checkbox,
  Container,
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
} from "@mui/material";
import React from "react";
import { useNavigate } from "react-router-dom";
import { NovaState } from "./App";
import { apiSet, apiSetValue } from "./api";

export const SettingsPage = ({
  state,
  setState,
  handleRefresh,
}: {
  state: NovaState;
  setState: React.Dispatch<React.SetStateAction<NovaState>>;
  handleRefresh: () => void;
}) => {
  const navigate = useNavigate();

  const handleBack = () => {
    navigate("/");
  };

  const handleEnabledContentChange = (value: {
    index: number;
    name: string;
  }) => {
    const add = state.enabledContent.indexOf(value) === -1;
    const enabledContent = add
      ? state.enabledContent.concat(value).sort((a, b) => a.index - b.index)
      : state.enabledContent.filter((item) => item.index !== value.index);
    const indices = enabledContent.map((value) => value.index).join(" ");
    apiSetValue("enabled-content-indices", indices);
    setState((prevState) => ({ ...prevState, enabledContent: enabledContent }));
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

  const getEnabledContent = () => {
    if (state.enabledContent.length) {
      return state.enabledContent;
    } else {
      return [];
    }
  };

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

        <InputLabel id="select-enabled-content-label" sx={{ mb: 1 }}>
          Select Enabled Content
        </InputLabel>
        <Box
          sx={{
            border: "1px solid",
            borderColor: "divider",
            borderRadius: 1,
            px: 1,
            py: 0,
            maxHeight: "16em",
            overflow: "auto",
            mb: 2,
          }}
        >
          <List dense>
            {state.availableContent.map((option) => (
              <ListItem key={option.index} disablePadding>
                <ListItemButton
                  dense
                  disableRipple
                  onClick={() => handleEnabledContentChange(option)}
                >
                  <ListItemIcon>
                    <Checkbox
                      disableRipple
                      checked={getEnabledContent().some(
                        (item: { index: number; name: string }) =>
                          item.index === option.index,
                      )}
                    />
                  </ListItemIcon>
                  <ListItemText primary={option.name} />
                </ListItemButton>
              </ListItem>
            ))}
          </List>{" "}
        </Box>

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

        <InputLabel id="network-settings-label" sx={{ mb: 2 }}>
          Ethernet Settings (Reload Server to Apply Changes)
        </InputLabel>
        <Stack spacing={2} direction="row" sx={{ mb: 8 }}>
          <TextField
            fullWidth
            label="Ethernet Interface"
            value={state.ethernetInterface}
            onChange={handleEthernetInterfaceChange}
          />
          <TextField
            fullWidth
            label="Module Address"
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
};

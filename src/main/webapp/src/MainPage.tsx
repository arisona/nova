import {
  Contrast,
  Palette,
  Settings,
  Speed,
  Sync,
  WbSunny,
} from "@mui/icons-material";
import {
  Box,
  Container,
  IconButton,
  SelectChangeEvent,
  Stack,
  Typography,
} from "@mui/material";
import * as React from "react";

import { NovaState, defaultNovaState } from "./App";
import { NovaColor } from "./NovaColor";
import { NovaSelectContent } from "./NovaContent";
import { NovaSlider } from "./NovaSlider";
import { apiGetState, apiSetValue } from "./api";
import { hsvToRgb } from "./color";

import { useNavigate } from "react-router-dom";

// icons: https://fonts.google.com/icons?icon.set=Material+Icons

export function MainPage() {
  const navigate = useNavigate();

  const [state, setState] = React.useState<NovaState>(defaultNovaState);

  const handleSelectedContentChange = (event: SelectChangeEvent) => {
    const selectedContentIndex = event.target.value as string;
    apiSetValue("selected-content", selectedContentIndex);
    setState((prevState) => ({
      ...prevState,
      selectedContentIndex: selectedContentIndex,
    }));
  };

  const handleBrightnessChange = (
    _event: Event,
    newValue: number | number[],
  ) => {
    apiSetValue("brightness", newValue as number);
    setState((prevState) => ({ ...prevState, brightness: newValue as number }));
  };

  const handleHueChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue("hue", newValue as number);
    setState((prevState) => ({ ...prevState, hue: newValue as number }));
  };

  const handleSaturationChange = (
    _event: Event,
    newValue: number | number[],
  ) => {
    apiSetValue("saturation", newValue as number);
    setState((prevState) => ({ ...prevState, saturation: newValue as number }));
  };

  const handleSpeedChange = (_event: Event, newValue: number | number[]) => {
    apiSetValue("speed", newValue as number);
    setState((prevState) => ({ ...prevState, speed: newValue as number }));
  };

  const handleRefresh = () => {
    apiGetState().then((state) => setState(state));
  };

  const handleSettings = () => {
    navigate("/settings");
  };

  const rgb = hsvToRgb(state.hue, state.saturation, state.brightness);

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
            NOVA
          </Typography>
          <Stack direction="row" alignItems="center">
            <IconButton aris-able="Refresh" onClick={handleRefresh}>
              <Sync />
            </IconButton>
            <IconButton aris-able="Settings" onClick={handleSettings}>
              <Settings />
            </IconButton>
          </Stack>
        </Stack>

        <NovaSelectContent
          enabledContent={state.enabledContent}
          selectedContentIndex={state.selectedContentIndex}
          handleContentChange={handleSelectedContentChange}
        />

        <NovaColor r={rgb[0]} g={rgb[1]} b={rgb[2]} />

        <NovaSlider
          icon={<WbSunny />}
          label="Brightness"
          value={state.brightness}
          onChange={handleBrightnessChange}
        />
        <NovaSlider
          icon={<Palette />}
          label="Hue"
          value={state.hue}
          onChange={handleHueChange}
        />
        <NovaSlider
          icon={<Contrast />}
          label="Saturation"
          value={state.saturation}
          onChange={handleSaturationChange}
        />
        <NovaSlider
          icon={<Speed />}
          label="Speed"
          value={state.speed}
          onChange={handleSpeedChange}
        />
      </Box>
    </Container>
  );
}

import { Contrast, Palette, Speed, WbSunny } from '@mui/icons-material';
import { Box, Button, Container, SelectChangeEvent, Stack, Typography } from '@mui/material';
import * as React from 'react';

import NovaColor from './NovaColor';
import NovaContent from './NovaContent';
import NovaSlider from './NovaSlider';
import { apiGetState, apiSet, apiSetValue } from './api';
import { hsvToRgb } from './color';

// icons: https://fonts.google.com/icons?icon.set=Material+Icons

export type NovaState = {
    availableContent: string[];
    selectedContent: string;
    brightness: number;
    hue: number;
    saturation: number;
    speed: number;
};

export const defaultNovaState: NovaState = {
    availableContent: ['No Content'],
    selectedContent: '0',
    brightness: 1,
    hue: 0.5,
    saturation: 1,
    speed: 0.5
};

export default function App() {
    const [state, setState] = React.useState<NovaState>(defaultNovaState);

    const handleSelectedContentChange = (event: SelectChangeEvent) => {
        const selectedContent = event.target.value as string;
        apiSetValue('selected-content', selectedContent);
        setState(prevState => ({ ...prevState, selectedContent: selectedContent }));
    };

    const handleBrightnessChange = (_event: Event, newValue: number | number[]) => {
        apiSetValue('brightness', newValue as number);
        setState(prevState => ({ ...prevState, brightness: newValue as number}));
    };

    const handleHueChange = (_event: Event, newValue: number | number[]) => {
        apiSetValue('hue', newValue as number);
        setState(prevState => ({ ...prevState, hue: newValue as number }));
    };

    const handleSaturationChange = (_event: Event, newValue: number | number[]) => {
        apiSetValue('saturation', newValue as number);
        setState(prevState => ({ ...prevState, saturation: newValue as number }));
    };

    const handleSpeedChange = (_event: Event, newValue: number | number[]) => {
        apiSetValue('speed', newValue as number);
        setState(prevState => ({ ...prevState, speed: newValue as number }));
    };

    const handleRefresh = () => {
        apiGetState().then(state => setState(state));
    }

    const handleReset = () => {
        apiSet('reset');
    }

    const handleReload = () => {
        apiSet('reload');
    }

    const rgb = hsvToRgb(state.hue, state.saturation, state.brightness);

    React.useEffect(() => handleRefresh(), []);

    return (
        <Container maxWidth="sm">
            <Box sx={{ my: 4 }}>
                <Typography variant="h3" component="h3" align="center" sx={{ mb: 2 }}>
                    NOVA
                </Typography>

                <NovaContent availableContent={state.availableContent} selectedContent={state.selectedContent} handleContentChange={handleSelectedContentChange} />

                <NovaColor r={rgb[0]} g={rgb[1]} b={rgb[2]} />

                <NovaSlider icon={<WbSunny />} label="Brightness" value={state.brightness} onChange={handleBrightnessChange} />
                <NovaSlider icon={<Palette />} label="Hue" value={state.hue} onChange={handleHueChange} />
                <NovaSlider icon={<Contrast />} label="Saturation" value={state.saturation} onChange={handleSaturationChange} />
                <NovaSlider icon={<Speed />} label="Speed" value={state.speed} onChange={handleSpeedChange} />

                <Stack spacing={2} direction="row" sx={{ mb: 4 }} alignItems="center">
                    <Button fullWidth variant="outlined" onClick={handleRefresh}>Refresh Content</Button>
                    <Button fullWidth variant="outlined" onClick={handleReset}>Reset Server</Button>
                    <Button fullWidth variant="outlined" onClick={handleReload}>Reload Server</Button>
                </Stack>

            </Box>
        </Container>
    );
}

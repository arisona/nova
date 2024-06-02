import {
  NavigateBefore,
  CheckBox,
  CheckBoxOutlineBlank,
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
import { useNavigate } from "react-router-dom";
import { apiSet } from "./api";

const availableContent = [
  "Balls",
  "Circles",
  "Circles2",
  "Circles3",
  "Circles4",
];

export function SettingsPage() {
  const navigate = useNavigate();

  const handleBack = () => {
    navigate("/");
  };

  const handleRestore = () => {
    apiSet("restore");
  };

  const handleReset = () => {
    apiSet("reset");
  };

  const handleReload = () => {
    apiSet("reload");
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
          <Stack direction="row" alignItems="center">
            <IconButton aris-able="Back" onClick={handleBack}>
              <NavigateBefore />
            </IconButton>
          </Stack>
        </Stack>

        <Stack direction="row" sx={{ mb: 4 }} alignItems="center">
          <Autocomplete
            fullWidth
            id="select-enabled-content"
            multiple
            limitTags={2}
            disableCloseOnSelect
            options={availableContent}
            renderTags={() => null}
            renderInput={(params) => (
              <TextField {...params} label="Select Enabled Content" />
            )}
            renderOption={(props, option, { selected }) => (
              <li {...props}>
                <Checkbox
                  icon={<CheckBoxOutlineBlank fontSize="small" />}
                  checkedIcon={<CheckBox fontSize="small" />}
                  style={{ marginRight: 8 }}
                  checked={selected}
                />
                {option}
              </li>
            )}
          />
        </Stack>

        <Stack spacing={2} direction="row" sx={{ mb: 4 }} alignItems="center">
          <FormGroup>
            <FormControlLabel
              control={<Switch defaultChecked />}
              label="Flip Content Vertically"
            />
          </FormGroup>
        </Stack>

        <Stack spacing={2} direction="row" sx={{ mb: 8 }} alignItems="center">
          <TextField fullWidth label="Ethernet Interface" />
          <TextField fullWidth label="Module Ethernet Address" />
        </Stack>

        <Stack spacing={2} direction="row" sx={{ mb: 4 }} alignItems="center">
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

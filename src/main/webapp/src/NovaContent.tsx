import {
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  SelectChangeEvent,
} from "@mui/material";

interface NovaSelectContentProps {
  enabledContent: string[];
  selectedContentIndex: string;
  handleContentChange: (event: SelectChangeEvent) => void;
}

export function NovaSelectContent({
  enabledContent,
  selectedContentIndex,
  handleContentChange,
}: NovaSelectContentProps) {
  return (
    <FormControl fullWidth sx={{ mb: 4 }}>
      <InputLabel id="select-content-label" sx={{ color: "primary.main" }}>
        Content
      </InputLabel>
      <Select
        sx={{
          color: "primary.main",
          "& .MuiOutlinedInput-notchedOutline": {
            borderColor: "primary.main",
          },
          "& .MuiSvgIcon-root": {
            color: "primary.main",
          },
        }}
        labelId="select-content-label"
        value={selectedContentIndex}
        label="Content"
        onChange={handleContentChange}
        variant="outlined"
      >
        {enabledContent.map((content, index) => {
          return (
            <MenuItem
              key={index}
              value={index.toString()}
              sx={{ color: "primary.main" }}
            >
              {content}
            </MenuItem>
          );
        })}
      </Select>
    </FormControl>
  );
}

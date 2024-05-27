import React from 'react';
import { FormControl, InputLabel, Select, MenuItem, SelectChangeEvent } from '@mui/material';

type NovaContentProps = {
    availableContent: string[]
    selectedContent: string;
    handleContentChange: (event: SelectChangeEvent) => void;
}

export default function NovaContent({ availableContent, selectedContent, handleContentChange }: NovaContentProps) {
    return (
        <FormControl fullWidth sx={{ mb: 4 }}>
            <InputLabel id="select-content-label" sx={{ color: 'primary.main' }}>Content</InputLabel>
            <Select
                sx={{
                    color: 'primary.main',
                    '& .MuiOutlinedInput-notchedOutline': {
                        borderColor: 'primary.main'
                    },
                    '& .MuiSvgIcon-root': {
                        color: 'primary.main'
                    }
                }}
                labelId="select-content-label"
                value={selectedContent}
                label="Content"
                onChange={handleContentChange}
                variant='outlined'
            >
                {availableContent.map((content) => {
                    return (
                        <MenuItem key={content} value={content} sx={{ color: 'primary.main' }}>{content}</MenuItem>
                    );
                })}
            </Select>
        </FormControl>
    );
}

import { NovaState, NovaStatus, Palette, defaultNovaState } from './App';

type ApiCommand = 'restore' | 'reset' | 'reload';

interface ApiSettingValues {
  'enabled-content': string; // names, comma-separated
  'selected-content': string;
  brightness: number;
  volume: number;
  palette: string;
  heat: number;
  flow: number;
  form: number;
  void: number;
  'flip-vertical': boolean;
  'ethernet-interface': string;
  'module0-address': number;
}

export const apiSet = (id: ApiCommand) => {
  return fetch(`/api/${id}`);
};

type Listener = () => void;
const rejectionListeners = new Set<Listener>();

/** Calls `listener` whenever the server rejects a change, e.g. a name that no longer
 * exists. Returns a function that removes the listener. */
export const onApiRejected = (listener: Listener) => {
  rejectionListeners.add(listener);
  return () => {
    rejectionListeners.delete(listener);
  };
};

export const apiSetValue = <Setting extends keyof ApiSettingValues>(
  id: Setting,
  value: ApiSettingValues[Setting]
) => {
  void fetch(
    `/api/${encodeURIComponent(id)}?value=${encodeURIComponent(String(value))}`
  )
    .then((response) => {
      if (!response.ok) {
        rejectionListeners.forEach((listener) => {
          listener();
        });
      }
    })
    .catch((err: unknown) => {
      console.error('apiSetValue failed:', err);
    });
};

interface ApiStateResponse {
  'available-content': string[];
  'enabled-content': string[];
  'selected-content': string;
  'audio-enabled': boolean;
  brightness: number;
  volume: number;
  palettes: unknown;
  palette: string;
  heat: number;
  flow: number;
  form: number;
  void: number;
  'flip-vertical': boolean;
  'ethernet-interface': string;
  'module0-address': string;
}

interface ApiStatusResponse {
  'status-ok': boolean;
  'status-message': string;
  'audio-ok': boolean;
  'audio-message': string;
}

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.every((x) => typeof x === 'string');
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null;
}

/** Palettes from the server; malformed entries are skipped. */
function parsePalettes(value: unknown): Palette[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((entry: unknown) => {
    if (
      !isRecord(entry) ||
      typeof entry.name !== 'string' ||
      !Array.isArray(entry.colors)
    )
      return [];
    const colors = entry.colors.flatMap((color: unknown) =>
      isRecord(color) && typeof color.hex === 'string'
        ? [
            {
              name: typeof color.name === 'string' ? color.name : '',
              code: typeof color.code === 'string' ? color.code : '',
              hex: color.hex,
            },
          ]
        : []
    );
    return colors.length ? [{ name: entry.name, colors }] : [];
  });
}

/** The current state, or null if the server could not be reached. */
export const apiGetState = async (): Promise<NovaState | null> => {
  try {
    const response = await fetch('/api/get-state');
    if (!response.ok) throw new Error(String(response.status));
    const data: unknown = await response.json();

    if (typeof data !== 'object' || data === null)
      throw new Error('Bad payload');
    const payload = data as Partial<ApiStateResponse>;

    if (!isStringArray(payload['available-content'])) {
      throw new Error('available-content missing');
    }
    const availableContent = payload['available-content'];

    // Safeguards: only keep names the server currently offers, and fall back to the
    // first entry if the selection is unknown.
    const enabledContent = (
      isStringArray(payload['enabled-content'])
        ? payload['enabled-content']
        : []
    ).filter((name) => availableContent.includes(name));
    const requestedContent = payload['selected-content'];
    const selectedContent =
      requestedContent !== undefined &&
      enabledContent.includes(requestedContent)
        ? requestedContent
        : (enabledContent[0] ?? '');

    const palettes = parsePalettes(payload.palettes);
    const requestedPalette = payload.palette;
    const palette =
      requestedPalette !== undefined &&
      palettes.some((entry) => entry.name === requestedPalette)
        ? requestedPalette
        : (palettes[0]?.name ?? '');

    const state: NovaState = {
      availableContent,
      enabledContent,
      selectedContent,
      audioEnabled: payload['audio-enabled'] ?? defaultNovaState.audioEnabled,
      brightness: payload.brightness ?? defaultNovaState.brightness,
      volume: payload.volume ?? defaultNovaState.volume,
      palettes,
      palette,
      heat: payload.heat ?? defaultNovaState.heat,
      flow: payload.flow ?? defaultNovaState.flow,
      form: payload.form ?? defaultNovaState.form,
      void: payload.void ?? defaultNovaState.void,
      flip: payload['flip-vertical'] ?? defaultNovaState.flip,
      ethernetInterface:
        payload['ethernet-interface'] ?? defaultNovaState.ethernetInterface,
      module0Address:
        payload['module0-address'] ?? defaultNovaState.module0Address,
    };

    return state;
  } catch (error) {
    console.error('Request failed: ', error);
    return null;
  }
};

export const apiGetStatus = async (): Promise<NovaStatus> => {
  try {
    const response = await fetch('/api/get-status');
    if (!response.ok) throw new Error(String(response.status));
    const data: unknown = await response.json();
    if (typeof data !== 'object' || data === null)
      throw new Error('Bad payload');
    const payload = data as Partial<ApiStatusResponse>;
    return {
      statusOk: payload['status-ok'] ?? false,
      statusMessage: payload['status-message'] ?? 'Unknown status',
      audioOk: payload['audio-ok'] ?? false,
      audioMessage: payload['audio-message'] ?? '',
    };
  } catch (error) {
    console.error('Request failed: ', error);
    return {
      statusOk: false,
      statusMessage: 'Cannot connect to the Nova server.',
      audioOk: false,
      audioMessage: '',
    };
  }
};

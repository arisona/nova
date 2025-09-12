import { NovaState, NovaStatus, defaultNovaState } from './App';

export const apiSet = (id: string) => {
  void fetch(`/api/${id}`);
};

export const apiSetValue = (id: string, value: string | number | boolean) => {
  void fetch(
    `/api/${encodeURIComponent(id)}?value=${encodeURIComponent(String(value))}`
  ).catch((err: unknown) => {
    console.error('apiSetValue failed:', err);
  });
};

interface ApiStateResponse {
  'available-content': string[];
  'enabled-content-indices': string[]; // indices as strings
  'selected-content-index': number;
  glow: number;
  tone: number;
  punch: number;
  flow: number;
  form: number;
  'flip-vertical': boolean;
  'cycle-duration': string;
  'ethernet-interface': string;
  'module0-address': string;
}

interface ApiStatusResponse {
  'status-ok': boolean;
  'status-message': string;
}

function isStringArray(v: unknown): v is string[] {
  return Array.isArray(v) && v.every((x) => typeof x === 'string');
}

export const apiGetState = async (): Promise<NovaState> => {
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

    const enabledIndicesRaw = payload['enabled-content-indices'] ?? [];
    const enabledIndices = Array.isArray(enabledIndicesRaw)
      ? enabledIndicesRaw
          .map((idx) => Number(idx))
          .filter((n) => Number.isFinite(n))
      : [];

    const state: NovaState = {
      availableContent: availableContent.map((name, index) => ({
        name,
        index,
      })),
      enabledContent: enabledIndices.map((index) => ({
        name: availableContent[index] ?? '',
        index,
      })),
      selectedContentIndex: payload['selected-content-index'] ?? -1,
      glow: payload.glow ?? defaultNovaState.glow,
      tone: payload.tone ?? defaultNovaState.tone,
      punch: payload.punch ?? defaultNovaState.punch,
      flow: payload.flow ?? defaultNovaState.flow,
      form: payload.form ?? defaultNovaState.form,
      flip: payload['flip-vertical'] ?? defaultNovaState.flip,
      cycleDuration:
        payload['cycle-duration'] ?? defaultNovaState.cycleDuration,
      ethernetInterface:
        payload['ethernet-interface'] ?? defaultNovaState.ethernetInterface,
      module0Address:
        payload['module0-address'] ?? defaultNovaState.module0Address,
    };

    return state;
  } catch (error) {
    console.error('Request failed: ', error);
    return defaultNovaState;
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
    };
  } catch (error) {
    console.error('Request failed: ', error);
    return {
      statusOk: false,
      statusMessage: 'Cannot connect to the Nova server.',
    };
  }
};

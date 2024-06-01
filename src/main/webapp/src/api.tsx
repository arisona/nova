import { NovaState, defaultNovaState } from "./App";

export function apiSet(id: string) {
  fetch(`/api/${id}`);
}

export function apiSetValue(id: string, value: number | string) {
  fetch(`/api/${id}?value=${value}`);
}

export async function apiGetState(): Promise<NovaState> {
  return fetch("/api/get-state")
    .then((response) => {
      if (!response.ok) throw new Error(response.status.toString());
      return response;
    })
    .then((response) => response.json())
    .then((data) => {
      const state: NovaState = {
        availableContent: data["available-content"] as string[],
        selectedContent: data["selected-content"] as string,
        brightness: data["brightness"] as number,
        hue: data["hue"] as number,
        saturation: data["saturation"] as number,
        speed: data["speed"] as number,
      };
      return state;
    })
    .catch(function (error) {
      console.error("Request failed: ", error);
      return defaultNovaState;
    });
}

import { NovaState, defaultNovaState } from "./App";

export const apiSet = (id: string) => {
  fetch(`/api/${id}`);
};

export const apiSetValue = (id: string, value: string | number | boolean) => {
  fetch(`/api/${id}?value=${value}`);
};

export const apiGetState = async (): Promise<NovaState> => {
  return fetch("/api/get-state")
    .then((response) => {
      if (!response.ok) throw new Error(response.status.toString());
      return response;
    })
    .then((response) => {
      return response.json();
    })
    .then((data) => {
      const availableContent = data["available-content"] as string[];
      const state: NovaState = {
        availableContent: availableContent.map(
          (name: string, index: number) => ({
            name: name as string,
            index: index as number,
          }),
        ),
        enabledContent: data["enabled-content-indices"].map(
          (index: string) => ({
            name: availableContent[+index] as string,
            index: +index,
          }),
        ),
        selectedContentIndex: data["selected-content-index"] as number,
        brightness: data["brightness"] as number,
        hue: data["hue"] as number,
        saturation: data["saturation"] as number,
        speed: data["speed"] as number,
        flip: data["flip"] as boolean,
        cycleDuration: data["cycle-duration"] as number,
        ethernetInterface: data["ethernet-interface"] as string,
        ethernetAddress: data["ethernet-address"] as string,
      };
      console.log(state);
      return state;
    })
    .catch((error) => {
      console.error("Request failed: ", error);
      return defaultNovaState;
    });
};

/** Tauri v2 global API type declarations. */

interface TauriEvent<T> {
  payload: T;
}

interface Window {
  __TAURI__: {
    core: {
      invoke<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T>;
    };
    event: {
      listen<T>(
        event: string,
        handler: (event: TauriEvent<T>) => void
      ): Promise<() => void>;
    };
  };
}

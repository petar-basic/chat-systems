const isDev = import.meta.env.DEV;

export const logger = {
  trace(className: string, method: string, message: string): void {
    if (isDev) console.debug(`[TRACE] ${className}.${method}: ${message}`);
  },
  info(className: string, method: string, message: string): void {
    console.info(`[INFO] ${className}.${method}: ${message}`);
  },
  warn(className: string, method: string, message: string): void {
    console.warn(`[WARN] ${className}.${method}: ${message}`);
  },
  error(className: string, method: string, error: unknown): void {
    console.error(`[ERROR] ${className}.${method}:`, error);
  },
} as const;

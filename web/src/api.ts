const API_URL = import.meta.env.VITE_API_URL || "/api";

export const fetchApi = (path: string, opts?: RequestInit): Promise<Response> =>
  fetch(`${API_URL}${path}`, opts);

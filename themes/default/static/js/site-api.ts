export type ApiOptions = RequestInit & { body?: unknown };

type ApiError = Error & {
  status?: number;
  payload?: unknown;
  clientRequestId?: string;
  requestId?: string;
};

function logDebug(event: string, details?: Record<string, unknown>): void {
  console.debug('[InkForge][frontend][debug]', event, details || {});
}

function logError(event: string, details?: Record<string, unknown>): void {
  console.error('[InkForge][frontend][error]', event, details || {});
}

function normalizeBody(options: ApiOptions): RequestInit {
  const next: RequestInit = { ...options };
  const headers = new Headers(options.headers || {});
  const body = options.body;
  const isFormData = typeof FormData !== 'undefined' && body instanceof FormData;
  const isSerializable = body != null && typeof body !== 'string' && !isFormData && !(body instanceof URLSearchParams) && !(body instanceof Blob);

  if (isSerializable) {
    headers.set('Content-Type', 'application/json');
    next.body = JSON.stringify(body);
  } else {
    next.body = body as BodyInit | null | undefined;
  }

  next.headers = headers;
  return next;
}

function generateClientRequestId(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `req_${Date.now()}_${Math.random().toString(16).slice(2)}`;
}

async function parseResponse<T>(response: Response, clientRequestId: string): Promise<T> {
  const contentType = response.headers.get('content-type') || '';
  const isJson = contentType.includes('application/json');
  const payload = isJson ? await response.json().catch(() => null) : null;
  const requestId = payload?.request_id || response.headers.get('x-request-id') || response.headers.get('x-client-request-id') || undefined;

  if (response.ok && payload?.code === 0) {
    logDebug('api_response_ok', {
      status: response.status,
      clientRequestId,
      requestId,
    });
    return payload.data as T;
  }

  const error = new Error(payload?.message || `Request failed with status ${response.status}`) as ApiError;
  error.status = response.status;
  error.payload = payload;
  error.clientRequestId = clientRequestId;
  error.requestId = requestId;
  logError('api_response_error', {
    status: response.status,
    clientRequestId,
    requestId,
    payload,
  });
  throw error;
}

export async function apiRequest<T>(path: string, options: ApiOptions = {}): Promise<T> {
  const request = normalizeBody(options);
  const clientRequestId = generateClientRequestId();
  const headers = new Headers(request.headers || {});
  headers.set('X-Client-Request-Id', clientRequestId);
  request.headers = headers;

  logDebug('api_request_start', {
    path,
    method: request.method || 'GET',
    clientRequestId,
  });
  const response = await fetch(path, {
    credentials: 'same-origin',
    ...request,
  });
  return parseResponse<T>(response, clientRequestId);
}

export function sanitizeRedirect(target: string | null | undefined, fallback = '/profile'): string {
  return typeof target === 'string' && target.startsWith('/') && !target.startsWith('//') ? target : fallback;
}

export function login(data: { login: string; password: string }) {
  return apiRequest('/api/v1/auth/login', { method: 'POST', body: data });
}

export function register(data: { username: string; email: string; password: string; display_name?: string | null }) {
  return apiRequest('/api/v1/auth/register', { method: 'POST', body: data });
}

export function logout() {
  return apiRequest('/api/v1/auth/logout', { method: 'POST' });
}

export function getMe<T>() {
  return apiRequest<T>('/api/v1/me');
}

export async function checkLoginStatus<T>(): Promise<T | null> {
  try {
    return await getMe<T>();
  } catch {
    return null;
  }
}

(function () {
  function logDebug(event, details) {
    console.debug('[Colophon][frontend][debug]', event, details || {});
  }

  function logError(event, details) {
    console.error('[Colophon][frontend][error]', event, details || {});
  }

  function normalizeBody(options) {
    const next = { ...options };
    const headers = new Headers(options.headers || {});
    const body = options.body;
    const isFormData = typeof FormData !== 'undefined' && body instanceof FormData;
    const isSerializable = body != null && typeof body !== 'string' && !isFormData && !(body instanceof URLSearchParams) && !(body instanceof Blob);

    if (isSerializable) {
      headers.set('Content-Type', 'application/json');
      next.body = JSON.stringify(body);
    }

    next.headers = headers;
    return next;
  }

  function generateClientRequestId() {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
      return crypto.randomUUID();
    }
    return `req_${Date.now()}_${Math.random().toString(16).slice(2)}`;
  }

  async function parseResponse(response, clientRequestId) {
    const contentType = response.headers.get('content-type') || '';
    const isJson = contentType.includes('application/json');
    const payload = isJson ? await response.json().catch(() => null) : null;
    const requestId = (payload && payload.request_id) || response.headers.get('x-request-id') || response.headers.get('x-client-request-id') || undefined;

    if (response.ok && payload && payload.code === 0) {
      logDebug('api_response_ok', {
        status: response.status,
        clientRequestId,
        requestId,
      });
      return payload.data;
    }

    const error = new Error(payload?.message || `Request failed with status ${response.status}`);
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

  async function apiRequest(path, options = {}) {
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
    return parseResponse(response, clientRequestId);
  }

  function sanitizeRedirect(target, fallback = '/profile') {
    return typeof target === 'string' && target.startsWith('/') && !target.startsWith('//') ? target : fallback;
  }

  function login(data) {
    return apiRequest('/api/v1/auth/login', { method: 'POST', body: data });
  }

  function register(data) {
    return apiRequest('/api/v1/auth/register', { method: 'POST', body: data });
  }

  function logout() {
    return apiRequest('/api/v1/auth/logout', { method: 'POST' });
  }

  function getMe() {
    return apiRequest('/api/v1/me');
  }

  async function checkLoginStatus() {
    try {
      return await getMe();
    } catch {
      return null;
    }
  }

  window.ColophonApi = {
    apiRequest,
    checkLoginStatus,
    getMe,
    login,
    logout,
    register,
    sanitizeRedirect,
  };
})();

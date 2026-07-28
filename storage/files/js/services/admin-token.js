export const hasValidAdminToken = () => {
  try {
    const cookie = document.cookie.split('; ').find((c) => c.startsWith('access_token='));
    if (cookie) {
      const token = cookie.split('=').slice(1).join('=');
      const payload = JSON.parse(atob(token.split('.')[1]));
      const scopes = (payload.scope ?? '').split(' ');
      const expired = payload.exp ? payload.exp * 1000 < Date.now() : false;
      return scopes.includes('user') && !expired;
    }
    return false;
  } catch (_err) {
    return false;
  }
};

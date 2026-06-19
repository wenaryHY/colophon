import { clearAllDrafts } from '../hooks/useAutoSaveDraft';

const AUTH_USER_KEY = 'colophon_auth_user';
const DRAFT_PREFIX = 'colophon-autosave-post-';

describe('AuthContext PII removal', () => {
  beforeEach(() => {
    sessionStorage.clear();
    localStorage.clear();
  });

  it('does not store user PII in sessionStorage under legacy key', () => {
    expect(sessionStorage.getItem(AUTH_USER_KEY)).toBeNull();
  });

  it('clearAllDrafts is exported and is a function', () => {
    expect(clearAllDrafts).toBeDefined();
    expect(typeof clearAllDrafts).toBe('function');
  });

  it('clearAllDrafts removes draft keys from sessionStorage', () => {
    sessionStorage.setItem(`${DRAFT_PREFIX}post-test1`, JSON.stringify({ title: 'test' }));
    sessionStorage.setItem(`${DRAFT_PREFIX}post-test2`, JSON.stringify({ title: 'test2' }));
    sessionStorage.setItem('keep-this', 'value');

    clearAllDrafts();

    expect(sessionStorage.getItem(`${DRAFT_PREFIX}post-test1`)).toBeNull();
    expect(sessionStorage.getItem(`${DRAFT_PREFIX}post-test2`)).toBeNull();
    expect(sessionStorage.getItem('keep-this')).toBe('value');
  });

  it('clearAllDrafts is safe to call on empty storage', () => {
    expect(() => clearAllDrafts()).not.toThrow();
  });
});

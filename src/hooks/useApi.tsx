import { useState, useEffect, useCallback, useRef } from 'react';
import * as api from '../api';

interface AsyncState<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
}

function useAsync<T>(fetcher: () => Promise<T>): AsyncState<T> & { refetch: () => void } {
  const [state, setState] = useState<AsyncState<T>>({ data: null, loading: true, error: null });
  const fetcherRef = useRef(fetcher);
  fetcherRef.current = fetcher;

  const fetch = useCallback(() => {
    setState(s => ({ ...s, loading: true, error: null }));
    fetcherRef.current()
      .then(data => setState({ data, loading: false, error: null }))
      .catch(err => setState(s => ({ ...s, data: null, loading: false, error: err.message })));
  }, []);

  useEffect(() => { fetch(); }, [fetch]);

  return { ...state, refetch: fetch };
}

export function useStatus() {
  return useAsync(() => api.fetchStatus());
}

export function useBackend() {
  return useAsync(() => api.fetchBackend());
}

export function useModels() {
  return useAsync(() => api.fetchModels());
}

export function useLogs(autoRefresh: boolean) {
  const [data, setData] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetch = useCallback(() => {
    api.fetchLogs()
      .then(lines => { setData(lines); setError(null); })
      .catch(err => setError(err.message))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    fetch();
    if (intervalRef.current) clearInterval(intervalRef.current);
    if (autoRefresh) {
      intervalRef.current = setInterval(fetch, 2000);
    }
    return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
  }, [autoRefresh, fetch]);

  return { data, loading, error, refetch: fetch };
}

export function useAuth() {
  return useAsync(() => api.fetchAuthFile());
}

export function useProfiles() {
  return useAsync(() => api.fetchProfiles());
}

export function useProxyConfig() {
  return useAsync(() => api.fetchProxyConfig());
}

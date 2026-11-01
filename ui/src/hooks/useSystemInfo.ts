import { useState, useEffect, useCallback } from 'react';
import type { SystemInfo } from '../types';
import { getSystemInfo, downloadWhisperModel } from '../lib/ipc';

export function useSystemInfo() {
  const [systemInfo, setSystemInfo] = useState<SystemInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [downloadingModel, setDownloadingModel] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const info = await getSystemInfo();
      setSystemInfo(info);
    } catch (err) {
      console.error('Failed to load system info:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const downloadModel = async (name: string) => {
    setDownloadingModel(name);
    try {
      await downloadWhisperModel(name);
      await refresh();
    } catch (err) {
      console.error(`Failed to download model ${name}:`, err);
    } finally {
      setDownloadingModel(null);
    }
  };

  return {
    systemInfo,
    loading,
    refresh,
    downloadModel,
    downloadingModel,
  };
}

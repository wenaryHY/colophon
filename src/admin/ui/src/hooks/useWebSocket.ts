import { useCallback, useEffect, useRef } from 'react';
import { useAuth } from '../contexts/AuthContext';

type WSEventType = 'comment_created' | 'comment_approved' | 'comment_deleted';
type WSHandler = (event: { type: WSEventType }) => void;

export function useWebSocket(onEvent?: WSHandler) {
  const { user } = useAuth();
  const wsRef = useRef<WebSocket | null>(null);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const attemptsRef = useRef(0);
  const onEventRef = useRef(onEvent);
  onEventRef.current = onEvent;

  const close = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close();
      wsRef.current = null;
    }
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    attemptsRef.current = 0;
  }, []);

  const connect = useCallback(() => {
    close();
    if (!user) return;

    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    wsRef.current = new WebSocket(`${protocol}//${location.host}/ws/admin`);

    wsRef.current.onopen = () => {
      console.log('[InkForge WS] 已连接');
      attemptsRef.current = 0;
      if (timerRef.current) {
        clearTimeout(timerRef.current);
        timerRef.current = null;
      }
    };

    wsRef.current.onmessage = (event) => {
      try {
        const payload = JSON.parse(event.data);
        console.log('[InkForge WS] 收到事件:', payload.type);
        onEventRef.current?.(payload as { type: WSEventType });
      } catch (error) {
        console.error('[InkForge WS] 解析消息失败:', error);
      }
    };

    wsRef.current.onclose = () => {
      console.log('[InkForge WS] 连接断开');
      wsRef.current = null;
      if (!user || timerRef.current) return;
      const delay = Math.min(1000 * Math.pow(2, attemptsRef.current), 30000);
      attemptsRef.current += 1;
      timerRef.current = setTimeout(() => {
        timerRef.current = null;
        connect();
      }, delay);
    };
  }, [close, user]);

  useEffect(() => {
    connect();
    return () => close();
  }, [connect, close]);

  return { reconnect: connect, close };
}

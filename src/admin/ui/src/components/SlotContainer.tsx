import { useEffect, useRef, useState, useCallback } from 'react';
import type { SlotInfo } from '../lib/slots';

interface SlotContainerProps {
  slot: SlotInfo;
  context?: Record<string, unknown>;
}

/** Iframe 包装器：安全握手 + postMessage 通信 */
export function SlotContainer({ slot, context }: SlotContainerProps) {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const tokenRef = useRef(crypto.randomUUID());
  const [height, setHeight] = useState(slot.height || 200);

  // targetOrigin 用 "*"：移除 allow-same-origin 后 iframe 获得 opaque origin，
  // 无法用具体 origin 匹配；token 握手机制已保证消息不会被非目标 iframe 接收
  const sendToIframe = useCallback((msg: Record<string, unknown>) => {
    iframeRef.current?.contentWindow?.postMessage(
      { ...msg, token: tokenRef.current },
      '*',
    );
  }, []);

  useEffect(() => {
    // 移除 allow-same-origin 后 iframe 获得 opaque origin，event.origin 为 "null"；
    // 放行 "null" origin，由 token 保证消息来源可信
    const handler = (event: MessageEvent) => {
      if (event.origin !== window.location.origin && event.origin !== 'null') return;
      const msg = event.data;
      if (!msg || msg.token !== tokenRef.current) return;

      switch (msg.type) {
        case 'resize':
          if (typeof msg.height === 'number') {
            setHeight(msg.height);
          }
          break;
        case 'navigate':
          if (typeof msg.path === 'string') {
            window.location.hash = msg.path;
          }
          break;
        case 'ready':
          sendToIframe({ type: 'context', data: context || {} });
          break;
      }
    };

    window.addEventListener('message', handler);

    const iframe = iframeRef.current;
    if (iframe) {
      iframe.onload = () => {
        sendToIframe({ type: 'init' });
      };
    }

    return () => {
      window.removeEventListener('message', handler);
    };
  }, [sendToIframe, context, slot.plugin_name]);

  return (
    <iframe
      ref={iframeRef}
      src={slot.entry}
      style={{
        width: slot.width || '100%',
        height: `${height}px`,
        border: 'none',
        overflow: 'hidden',
      }}
      sandbox="allow-scripts"
      title={slot.label}
    />
  );
}

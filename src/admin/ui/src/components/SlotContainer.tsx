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

  const sendToIframe = useCallback((msg: Record<string, unknown>) => {
    iframeRef.current?.contentWindow?.postMessage(
      { ...msg, token: tokenRef.current },
      window.location.origin,
    );
  }, []);

  useEffect(() => {
    const handler = (event: MessageEvent) => {
      if (event.origin !== window.location.origin) return;
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
      sandbox="allow-scripts allow-same-origin"
      title={slot.label}
    />
  );
}

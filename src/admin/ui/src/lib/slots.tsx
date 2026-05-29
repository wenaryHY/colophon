import { createContext, useContext } from 'react';
import { SlotContainer } from '../components/SlotContainer';

export interface SlotInfo {
  target: string;
  label: string;
  entry: string;
  width?: number;
  height?: number;
  plugin_name: string;
}

interface SlotsContextValue {
  slots: SlotInfo[];
}

export const SlotsContext = createContext<SlotsContextValue>({ slots: [] });

export function useSlots() {
  return useContext(SlotsContext);
}

/** 按 target 筛选并渲染匹配的插槽 */
export function SlotRenderer({ target, context }: { target: string; context?: Record<string, unknown> }) {
  const { slots } = useSlots();
  const matched = slots.filter(s => s.target === target);
  if (matched.length === 0) return null;
  return (
    <>
      {matched.map(s => (
        <SlotContainer key={s.plugin_name} slot={s} context={context} />
      ))}
    </>
  );
}

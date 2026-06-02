import { useState, useEffect } from 'react';

const BREAKPOINT_TABLET = 768;
const BREAKPOINT_DESKTOP = 1024;

export function useResponsive() {
  const [width, setWidth] = useState(window.innerWidth);

  useEffect(() => {
    const handler = () => setWidth(window.innerWidth);
    window.addEventListener('resize', handler);
    return () => window.removeEventListener('resize', handler);
  }, []);

  return {
    width,
    isMobile: width < BREAKPOINT_TABLET,
    isTablet: width >= BREAKPOINT_TABLET && width < BREAKPOINT_DESKTOP,
    isDesktop: width >= BREAKPOINT_DESKTOP,
  };
}

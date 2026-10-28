// The one place the mobile breakpoint is written in TypeScript. It is device
// tuning, not semantics (spec/gui-semantics.md: "the breakpoint at which the
// layout switches ... is tuning"), and must agree with the `768px` media
// queries in src/styles.

const MOBILE_BREAKPOINT_PX = 768;

export const isMobileViewport = (): boolean => window.innerWidth <= MOBILE_BREAKPOINT_PX;

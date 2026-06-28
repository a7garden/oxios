// Thin compatibility shim — kept so legacy imports of `TypingIndicator`
// from this module path continue to resolve. The component now lives at
// `./live-activity-bar` and reads the chat store directly; new code
// should import `LiveActivityBar` from there.
export { LiveActivityBar as TypingIndicator } from './live-activity-bar'
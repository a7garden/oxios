/**
 * Notification sound utilities (RFC-028 SP-1d).
 *
 * Uses the Web Audio API with oscillator synthesis — no external audio files.
 * Different severity levels produce distinct tones:
 * - success: ascending C5→E5 (major third)
 * - error:   descending A4→F4 (minor sixth)
 * - warning: single A4
 * - info:    single E5 (short)
 */

import type { NotificationSeverity } from '@/stores/notifications'

let ctx: AudioContext | null = null

function getCtx(): AudioContext | null {
  if (typeof window === 'undefined') return null
  if (!ctx) {
    const Ctor =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext
    if (!Ctor) return null
    ctx = new Ctor()
  }
  // Browsers suspend AudioContext after inactivity; resume on demand.
  if (ctx.state === 'suspended') {
    void ctx.resume()
  }
  return ctx
}

/** Frequency in Hz for a given note name + octave. */
function noteToFreq(note: string, octave: number): number {
  const semitones: Record<string, number> = {
    c: 0,
    'c#': 1,
    d: 2,
    'd#': 3,
    e: 4,
    f: 5,
    'f#': 6,
    g: 7,
    'g#': 8,
    a: 9,
    'a#': 10,
    b: 11,
  }
  const midi = (octave + 1) * 12 + (semitones[note.toLowerCase()] ?? 9)
  return 440 * 2 ** ((midi - 69) / 12)
}

function tone(
  audioCtx: AudioContext,
  freq: number,
  startAt: number,
  duration: number,
  gainPeak: number,
) {
  const osc = audioCtx.createOscillator()
  const gain = audioCtx.createGain()
  osc.type = 'sine'
  osc.frequency.value = freq
  // Envelope: quick attack, exponential decay.
  gain.gain.setValueAtTime(0, startAt)
  gain.gain.linearRampToValueAtTime(gainPeak, startAt + 0.01)
  gain.gain.exponentialRampToValueAtTime(0.0001, startAt + duration)
  osc.connect(gain)
  gain.connect(audioCtx.destination)
  osc.start(startAt)
  osc.stop(startAt + duration)
}
/**
 * Play a notification sound for the given severity.
 * No-op if Web Audio API is unavailable or the user hasn't interacted
 * with the page yet (browser autoplay policy).
 */
export function playNotificationSound(severity: NotificationSeverity) {
  const audioCtx = getCtx()
  if (!audioCtx) return

  const t0 = audioCtx.currentTime
  const vol = 0.08

  switch (severity) {
    case 'success':
      // Ascending C5 → E5 (major third, uplifting).
      tone(audioCtx, noteToFreq('c', 5), t0, 0.15, vol)
      tone(audioCtx, noteToFreq('e', 5), t0 + 0.1, 0.2, vol)
      break
    case 'error':
      // Descending A4 → F4 (dissonant, attention-grabbing).
      tone(audioCtx, noteToFreq('a', 4), t0, 0.15, vol)
      tone(audioCtx, noteToFreq('f', 4), t0 + 0.12, 0.25, vol * 1.2)
      break
    case 'warning':
      // Single A4.
      tone(audioCtx, noteToFreq('a', 4), t0, 0.2, vol)
      break
    case 'info':
      // Short E5 blip.
      tone(audioCtx, noteToFreq('e', 5), t0, 0.1, vol * 0.7)
      break
  }
}

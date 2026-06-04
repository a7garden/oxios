/**
 * Emoji shortcodes for the `:` autocomplete in the knowledge editor.
 *
 * The autocomplete source in `markdown-editor.tsx` matches on text
 * after a `:` trigger. This module is the single source of truth for
 * the shortcode → Unicode glyph mapping.
 *
 * Scope: ~120 shortcodes covering the most common cases (gitmoji-style
 * + UI affordances). The full Unicode emoji pattern in `emoji.ts` is
 * for *detection* (is this string an emoji?), not for the shortcode
 * dictionary. The two are intentionally separate: shortcodes are a
 * convenience for human-typed markdown, while the pattern is used by
 * other systems (habit tracking, etc.) that work with raw Unicode.
 *
 * Format: keys are the shortcode (no leading `:`), values are the
 * actual glyph (or a ZWJ sequence, etc.) that gets inserted.
 */
export const EMOJI_SHORTCODES: Record<string, string> = {
  // Smileys
  smile: '😄',
  grinning: '😀',
  laugh: '😆',
  rofl: '🤣',
  joy: '😂',
  wink: '😉',
  blush: '😊',
  slightly_smiling: '🙂',
  neutral_face: '😐',
  expressionless: '😑',
  no_mouth: '😶',
  smirk: '😏',
  unamused: '😒',
  rolling_eyes: '🙄',
  grimacing: '😬',
  lying_face: '🤥',
  relieved: '😌',
  pensive: '😔',
  sleepy: '😪',
  sleepy_face: '😴',
  // Emotions
  sob: '😭',
  cry: '😢',
  scream: '😱',
  confused: '😕',
  worried: '😟',
  angry: '😠',
  rage: '😡',
  triumph: '😤',
  // Gestures
  thumbsup: '👍',
  '+1': '👍',
  thumbsdown: '👎',
  '-1': '👎',
  ok_hand: '👌',
  wave: '👋',
  clap: '👏',
  raised_hands: '🙌',
  pray: '🙏',
  muscle: '💪',
  point_up: '☝️',
  point_down: '👇',
  point_left: '👈',
  point_right: '👉',
  // Hearts / symbols
  heart: '❤️',
  red_heart: '❤️',
  blue_heart: '💙',
  green_heart: '💚',
  yellow_heart: '💪',
  broken_heart: '💔',
  sparkles: '✨',
  star: '⭐',
  star2: '🌟',
  fire: '🔥',
  boom: '💥',
  zap: '⚡',
  // Status / task
  check: '✅',
  white_check_mark: '✅',
  x: '❌',
  negative_squared_cross_mark: '❎',
  bangbang: '‼️',
  warning: '⚠️',
  no_entry: '⛔',
  question: '❓',
  // Arrows
  arrow_up: '⬆️',
  arrow_down: '⬇️',
  arrow_left: '⬅️',
  arrow_right: '➡️',
  // Tools / objects
  bulb: '💡',
  hammer: '🔨',
  wrench: '🔧',
  gear: '⚙️',
  wrench_and_screwdriver: '🛠️',
  mag: '🔍',
  microscope: '🔬',
  telescope: '🔭',
  rocket: '🚀',
  rocket_emoji: '🚀',
  // Nature
  seed: '🌱',
  seedling: '🌱',
  tree: '🌳',
  herb: '🌿',
  // Food / drink
  coffee: '☕',
  tea: '🍵',
  beer: '🍺',
  wine: '🍷',
  // Tech
  computer: '💻',
  desktop: '🖥️',
  keyboard: '⌨️',
  package: '📦',
  // Communication
  envelope: '✉️',
  email: '✉️',
  // Activity
  tada: '🎉',
  party_popper: '🎉',
  confetti_ball: '🎊',
  balloon: '🎈',
  gift: '🎁',
  trophy: '🏆',
  medal: '🏅',
  // Travel
  car: '🚗',
  rocket_emoji_2: '🚀',
  airplane: '✈️',
  // Time
  hourglass: '⏳',
  watch: '⌚',
  clock: '🕐',
  calendar: '📅',
  // Books / docs
  books: '📚',
  book: '📖',
  notebook: '📓',
  memo: '📝',
  pencil: '✏️',
  // Money
  moneybag: '💰',
  dollar: '💵',
  credit_card: '💳',
  // Lock / security
  lock: '🔒',
  unlocked: '🔓',
  key: '🔑',
  shield: '🛡️',
  // Misc gitmoji-style
  bug: '🐛',
  ambulance: '🚑',
  lipstick: '💄',
  construction: '🚧',
  recycle: '♻️',
  // People
  baby: '👶',
  boy: '👦',
  girl: '👧',
  man: '👨',
  woman: '👩',
  // Tones (combined emoji — shortcode gives the default tone)
  raised_hand: '✋',
  ok_man: '🙆',
  no_good_man: '🙅',
  bowing_man: '🙇',
  // Common UI
  eyes: '👀',
  ear: '👂',
  nose: '👃',
  tongue: '👅',
  // Animals
  dog: '🐶',
  cat: '🐱',
  mouse: '🐭',
  hamster: '🐹',
  rabbit: '🐰',
  fox: '🦊',
  bear: '🐻',
  panda: '🐼',
  koala: '🐨',
  tiger: '🐯',
  lion: '🦁',
  cow: '🐮',
  pig: '🐷',
  frog: '🐸',
  monkey: '🐵',
  chicken: '🐔',
  penguin: '🐧',
  bird: '🐦',
  baby_chick: '🐤',
  // Weather
  sunny: '☀️',
  cloud: '☁️',
  rain: '🌧️',
  snow: '❄️',
  // Programming
  hash: '#️⃣',
  asterisk: '*️⃣',
  zero: '0️⃣',
  one: '1️⃣',
  two: '2️⃣',
  three: '3️⃣',
  four: '4️⃣',
  five: '5️⃣',
  six: '6️⃣',
  seven: '7️⃣',
  eight: '8️⃣',
  nine: '9️⃣',
}

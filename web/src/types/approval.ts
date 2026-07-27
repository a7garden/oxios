/**
 * RFC-035 approval mode system — TS mirror of the kernel types.
 * Backend: crates/oxios-kernel/src/approval/policy.rs
 */

/** User-selectable approval mode. Kebab-case on the wire (matches Rust serde). */
export type ApprovalMode = 'manual' | 'allow-list' | 'auto-run'

/** Tool policy tier. Lowercase on the wire. */
export type ToolPolicy = 'auto' | 'ondemand' | 'always'

/** Shape returned by GET /api/security/approval and accepted by PATCH. */
export interface ApprovalConfig {
  mode: ApprovalMode
  allow_list: string[]
  tool_overrides: Record<string, ToolPolicy>
}

/** PATCH body — all fields optional. */
export interface ApprovalConfigPatch {
  mode?: ApprovalMode
  allow_list?: string[]
  tool_overrides?: Record<string, ToolPolicy>
}

/** POST /api/security/approval/allow-list body. */
export interface GrantBody {
  key: string
}

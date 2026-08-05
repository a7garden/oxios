export enum FrameType { Noise = 0x01, App = 0x02, Ping = 0x03, Pong = 0x04, Close = 0x05 }
export const MAX_FRAME_SIZE = 65_536
export interface Frame { type: FrameType; payload: Uint8Array }

export function encodeFrame(type: FrameType, payload: Uint8Array): Uint8Array {
  if (payload.byteLength > MAX_FRAME_SIZE) throw new RangeError('frame payload exceeds 65536 bytes')
  const output = new Uint8Array(5 + payload.byteLength)
  output[0] = type
  new DataView(output.buffer).setUint32(1, payload.byteLength, false)
  output.set(payload, 5)
  return output
}

export function decodeFrame(data: ArrayBuffer | Uint8Array): Frame {
  const bytes = data instanceof Uint8Array ? data : new Uint8Array(data)
  if (bytes.byteLength < 5) throw new Error('incomplete frame header')
  const type = bytes[0] as FrameType
  if (type < FrameType.Noise || type > FrameType.Close) throw new Error(`unknown frame type: ${type}`)
  const size = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(1, false)
  if (size > MAX_FRAME_SIZE) throw new RangeError('frame payload exceeds 65536 bytes')
  if (bytes.byteLength !== size + 5) throw new Error('frame size mismatch')
  return { type, payload: bytes.slice(5) }
}

// Retained native u64 values must survive the JavaScript capture unchanged.
// The browser SDK's decimal revision remains the comparison surface for its view.
export function parseCaptureJSON(text) {
  if (typeof JSON.rawJSON !== 'function') throw new Error('capture needs exact JSON number support');
  return JSON.parse(text, (_key, value, context) => {
    if (typeof value === 'number' && Number.isInteger(value) && !Number.isSafeInteger(value)) {
      if (!context?.source) throw new Error('capture cannot retain the native integer exactly');
      return JSON.rawJSON(context.source);
    }
    return value;
  });
}

export function captureDecimal(value) {
  if (typeof value === 'string' && /^\d+$/.test(value)) return value;
  if (typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) return String(value);
  if (typeof JSON.isRawJSON === 'function' && JSON.isRawJSON(value) && /^\d+$/.test(value.rawJSON)) return value.rawJSON;
  throw new Error('capture revision is not an exact unsigned decimal');
}

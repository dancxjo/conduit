// Project a supplied wall-clock reading using the selected browser rule provider.
export function prepareBrowserCivilClock(timezone) {
  if (typeof timezone !== "string" || timezone.length > 64 ||
      !/^[A-Za-z0-9_+-]+(?:\/[A-Za-z0-9_+-]+)*$/.test(timezone)) {
    throw new RangeError("invalid civil clock timezone");
  }
  const format = new Intl.DateTimeFormat("en-US", {
    timeZone: timezone,
    calendar: "iso8601",
    numberingSystem: "latn",
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    timeZoneName: "longOffset",
  });

  return unixMillis => {
    if (!Number.isSafeInteger(unixMillis) || unixMillis < 0 || unixMillis > 253402300799999) {
      throw new RangeError("civil clock instant outside the supported Unix range");
    }
    const parts = Object.fromEntries(format.formatToParts(unixMillis)
      .filter(part => part.type !== "literal")
      .map(part => [part.type, part.value]));
    const date = `${parts.year.padStart(4, "0")}-${parts.month}-${parts.day}`;
    const time = `${parts.hour}:${parts.minute}:${parts.second}`;
    const offset = parts.timeZoneName === "GMT" ? 0 : offsetMinutes(parts.timeZoneName);
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date) || !/^\d{2}:\d{2}:\d{2}$/.test(time) ||
        Math.abs(offset) > 840) {
      throw new RangeError("civil clock projection outside the checked profile");
    }
    return {
      local_date: date,
      local_time: time,
      timezone,
      // ECMA-402 exposes the rule provider, without a portable tzdb revision.
      timezone_rule_set: "browser/intl-date-time-format",
      utc_offset_minutes: offset,
      observed_at_unix_ms: unixMillis,
    };
  };
}

function offsetMinutes(text) {
  const result = /^GMT([+-])(\d{2}):(\d{2})$/.exec(text);
  if (!result || Number(result[3]) > 59) {
    throw new RangeError("civil clock offset is not a whole number of minutes");
  }
  return (Number(result[2]) * 60 + Number(result[3])) * (result[1] === "-" ? -1 : 1);
}

// The planned timezone and the rule provider belong to the acquired clock.
// Its reply carries the observed instant and the corresponding civil labels.
export const CIVIL_CLOCK_REPLY_BYTES = 28;

export function prepareBrowserCivilClockReply(timezone) {
  const project = prepareBrowserCivilClock(timezone);
  const reply = new Uint8Array(CIVIL_CLOCK_REPLY_BYTES);
  const fields = new DataView(reply.buffer);
  const encoder = new TextEncoder();

  return unixMillis => {
    const reading = project(unixMillis);
    fields.setBigUint64(0, BigInt(reading.observed_at_unix_ms), true);
    fields.setInt16(8, reading.utc_offset_minutes, true);
    encoder.encodeInto(reading.local_date, reply.subarray(10, 20));
    encoder.encodeInto(reading.local_time, reply.subarray(20, 28));
    return reply;
  };
}

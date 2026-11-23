import { deepEqual, throws } from "node:assert/strict";
import { test } from "node:test";
import { prepareBrowserCivilClock } from "./browser-civil-clock.mjs";

test("Seattle civil labels retain the supplied instant across gaps and folds", () => {
  const project = prepareBrowserCivilClock("America/Los_Angeles");
  for (const [instant, date, time, offset] of [
    ["2026-03-08T09:59:59.876Z", "2026-03-08", "01:59:59", -480],
    ["2026-03-08T10:00:00.000Z", "2026-03-08", "03:00:00", -420],
    ["2026-11-01T08:59:59.000Z", "2026-11-01", "01:59:59", -420],
    ["2026-11-01T09:00:00.000Z", "2026-11-01", "01:00:00", -480],
    ["2024-03-01T07:59:59.000Z", "2024-02-29", "23:59:59", -480],
    ["2024-03-01T08:00:00.000Z", "2024-03-01", "00:00:00", -480],
  ]) {
    const millis = Date.parse(instant);
    deepEqual(project(millis), {
      local_date: date,
      local_time: time,
      timezone: "America/Los_Angeles",
      timezone_rule_set: "browser/intl-date-time-format",
      utc_offset_minutes: offset,
      observed_at_unix_ms: millis,
    });
  }
});

test("unknown timezones and instants outside the checked range refuse", () => {
  for (const timezone of ["", "Seattle", "../etc/localtime", "+05:00"]) {
    throws(() => prepareBrowserCivilClock(timezone), RangeError);
  }
  const project = prepareBrowserCivilClock("UTC");
  for (const instant of [-1, 0.5, Infinity, Number.MAX_SAFE_INTEGER]) {
    throws(() => project(instant), RangeError);
  }
  deepEqual(project(0), {
    local_date: "1970-01-01",
    local_time: "00:00:00",
    timezone: "UTC",
    timezone_rule_set: "browser/intl-date-time-format",
    utc_offset_minutes: 0,
    observed_at_unix_ms: 0,
  });
});

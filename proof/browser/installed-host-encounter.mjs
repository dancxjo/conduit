// Native Host preparation and browser Birth have no shared admission state.
// Retain both results before rendezvous; retire a ready Host if browser setup fails.
export async function prepareInstalledHostEncounter(prepareHost, prepareBrowser) {
  const [host, browser] = await Promise.allSettled([
    Promise.resolve().then(prepareHost),
    Promise.resolve().then(prepareBrowser),
  ]);
  if (host.status === "rejected") {
    if (browser.status === "rejected") {
      throw new AggregateError([host.reason, browser.reason], "Host and browser preparation failed");
    }
    throw host.reason;
  }
  if (browser.status === "rejected") {
    try { await host.value.close(); }
    catch (cleanup) {
      throw new AggregateError([browser.reason, cleanup], "Browser preparation and Host cleanup failed");
    }
    throw browser.reason;
  }
  return { installed: host.value, browser: browser.value };
}

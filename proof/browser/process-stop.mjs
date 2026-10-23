// Closing the pipes is part of retiring owned output, not merely observing process exit.
export function stopProcess(child, clock = {
  set: (fn, ms) => setTimeout(fn, ms), clear: timer => clearTimeout(timer),
}) {
  if (!child?.pid) return Promise.resolve();
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  const drained = stream => !stream || stream.readableEnded || stream.destroyed;
  if (exited() && drained(child.stdout) && drained(child.stderr)) return Promise.resolve();
  return new Promise((resolve, reject) => {
    let killTimer, exitTimer;
    const finish = error => {
      clock.clear(killTimer); clock.clear(exitTimer);
      child.off("exit", onExit); child.off("close", onClose);
      if (error) reject(error); else resolve();
    };
    const onExit = () => clock.clear(killTimer);
    const onClose = () => finish();
    child.once("exit", onExit);
    child.once("close", onClose);
    exitTimer = clock.set(() => finish(new Error("Host process or owned output did not close during cleanup")), 5_000);
    if (!exited()) {
      killTimer = clock.set(() => { try { child.kill("SIGKILL"); } catch (error) { finish(error); } }, 2_000);
      try { child.kill(); } catch (error) { finish(error); }
    }
  });
}

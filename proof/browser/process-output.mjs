// Readiness spends the enclosing encounter's absolute budget, never a fresh phase timeout.
export function processOutput(child, pattern, label, budget, clock = {
  now: () => performance.now(), set: (fn, ms) => setTimeout(fn, ms), clear: id => clearTimeout(id),
}) {
  const started = clock.now();
  let output = '', tail = '', truncated = false;
  const diagnostic = () => ({ label, elapsed_ms: clock.now() - started,
    encounter_elapsed_ms: clock.now() - budget.started, remaining_ms: budget.deadline - clock.now(),
    pid: child.pid ?? null, exit_code: child.exitCode, signal_code: child.signalCode,
    output, output_truncated: truncated });
  return new Promise((resolve, reject) => {
    let settled = false, timer;
    const finish = (error, match) => {
      if (settled) return;
      settled = true; clock.clear(timer);
      child.stdout.off('data', inspect); child.stderr.off('data', inspect);
      child.off('exit', onExit); child.off('error', onError);
      const record = { ...diagnostic(), outcome: error ? "refused" : "ready" }; budget.records.push(record);
      if (error) reject(new Error(`${label}: ${error}\n${JSON.stringify(record)}`));
      else resolve(match);
    };
    const inspect = chunk => {
      const text = chunk.toString();
      const available = 16 * 1024 - output.length;
      output += text.slice(0, available); truncated ||= text.length > available;
      if (clock.now() >= budget.deadline) return finish('encounter deadline expired');
      tail = (tail + text).slice(-16 * 1024);
      const match = tail.match(pattern);
      if (match) finish(null, match);
    };
    const onExit = status => finish(`exited before readiness (${status})`);
    const onError = error => finish(`process error: ${error.message}`);
    child.stdout.on('data', inspect); child.stderr.on('data', inspect);
    child.once('exit', onExit); child.once('error', onError);
    const remaining = budget.deadline - clock.now();
    if (remaining <= 0) finish('encounter deadline expired');
    else if (child.exitCode !== null || child.signalCode !== null) finish('already exited before readiness');
    else timer = clock.set(() => finish('encounter deadline expired'), remaining);
  });
}

export function onceCleanup(cleanup) {
  let pending;
  return () => pending ??= Promise.resolve().then(cleanup);
}

// Capture each test's ownership independently of later hooks or continuations.
export function preparationState(timeout, now = () => performance.now(), teardownClock = {
  set: (fn, ms) => setTimeout(fn, ms), clear: id => clearTimeout(id),
}) {
  const started = now();
  const state = { started, deadline: started + timeout, records: [], retired: false };
  const cleanups = [], pending = [];
  state.own = cleanup => {
    const close = onceCleanup(cleanup);
    cleanups.push(close);
    if (state.retired) void close().catch(() => {});
    return close;
  };
  state.check = () => {
    if (state.retired || now() >= state.deadline) throw new Error('Encounter preparation retired or deadline expired');
  };
  state.run = (label, prepare) => {
    const record = { label, outcome: 'pending', started_ms: now() - started };
    state.records.push(record);
    const task = Promise.resolve().then(() => { state.check(); return prepare(); }).then(async value => {
      try { state.check(); }
      catch (error) { if (value?.close) await value.close(); throw error; }
      record.outcome = 'ready'; return value;
    }).catch(error => { record.outcome = 'refused'; record.error = String(error).slice(0, 2048); throw error; })
      .finally(() => { record.elapsed_ms = now() - started - record.started_ms; record.encounter_elapsed_ms = now() - started; });
    pending.push(task); return task;
  };
  const retire = onceCleanup(async () => {
    const initial = cleanups.map(close => close());
    const cancelled = state.cancel ? Promise.resolve().then(state.cancel) : Promise.resolve();
    const join = async () => {
      const results = await Promise.allSettled([...initial, cancelled, ...pending]);
      // Late preparation may acquire a directory after retirement. Its registered
      // cleanup is started immediately and also joined here before teardown ends.
      const final = await Promise.allSettled(cleanups.map(close => close()));
      const failures = [...results.slice(0, initial.length + 1), ...final]
        .filter(result => result.status === 'rejected').map(result => result.reason);
      if (failures.length) throw new AggregateError(failures, 'Encounter cleanup failed');
    };
    let timer;
    try {
      await Promise.race([join(), new Promise((_, reject) => {
        timer = teardownClock.set(() => {
          state.records.push({ label: 'cleanup', outcome: 'refused', limit_ms: 6000,
            error: 'Encounter cleanup join deadline expired',
            pending: state.records.filter(record => record.outcome === 'pending').map(record => record.label) });
          reject(new Error('Encounter cleanup join deadline expired (6000 ms)'));
        }, 6000);
      })]);
    } finally { teardownClock.clear(timer); }
  });
  state.retire = () => { state.retired = true; return retire(); };
  return state;
}

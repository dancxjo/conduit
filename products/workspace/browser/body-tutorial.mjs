// Contextual beginner guidance projected from ordinary Body truth. This module
// retains no tutorial progress: the Body biography is the only progression.
export function deriveBodyTutorial({ current, evidence, playback }) {
  if (!current) return Object.freeze({ phase: 'birth', title: 'Birth one real Body',
    detail: 'Choose useful Forms. Birth creates the Body identity and biography you will keep operating.', action: 'birth' });
  const wakes = evidence?.evidence?.wakes ?? [];
  const failed = wakes.some(wake => wake.lifecycle === 'Failed');
  const wakeCount = wakes.length;
  if (current.state === 'FULFILLED') return Object.freeze({ phase: 'fulfilled', title: 'Its useful life is complete',
    detail: 'Fulfilled is terminal, not deletion. Open lifecycle evidence to inspect the closed biography.', action: 'inspect' });
  if (failed) return Object.freeze({ phase: 'repair', title: 'Inspect the real fault',
    detail: 'This biography contains a failed Wake. Look inside its lifecycle evidence, then change the actual workset or available Hosts before waking again.', action: 'inspect' });
  if (current.state === 'LULLED' && wakeCount === 0) return Object.freeze({ phase: 'wake', title: 'Wake this Body',
    detail: 'Birth made one retained Body. Wake admits its first exact Plan and Play without creating another Body.', action: 'wake' });
  if (current.state === 'LULLED') return Object.freeze({ phase: 'lull', title: 'Retained rest is not completion',
    detail: 'The Body is lulled: its identity, Forms, and biography remain. Wake it again, revise its Forms, or explicitly fulfill it when its purpose is complete.', action: 'wake' });
  if (current.workload_revision > 0) return Object.freeze({ phase: 'revised', title: 'One Body, a changed workset',
    detail: 'The workload revision changed without rebirth. The current Plan realizes the revised Forms for this same Body.', action: 'inspect' });
  if (wakeCount > 1) return Object.freeze({ phase: 'continuity', title: 'The same Body woke again',
    detail: 'A fresh Wake, Plan, and Play continue one retained biography. Use a Form again or inspect how this realization differs.', action: 'use' });
  return Object.freeze({ phase: 'living', title: 'Use it, then leave it useful',
    detail: playback?.state === 'Idle'
      ? 'Idle means admitted work is awaiting future input. Finite means bounded state, queues, authority, and obligations—not a short lifetime.'
      : 'Interact more than once. A finite Body may remain awake for minutes, months, or indefinitely because its instantaneous and retained bounds stay finite.',
    action: 'use' });
}

export function renderBodyTutorial(root, context) {
  const guidance = deriveBodyTutorial(context);
  root.dataset.tutorialPhase = guidance.phase;
  root.querySelector('[data-tutorial-title]').textContent = guidance.title;
  root.querySelector('[data-tutorial-detail]').textContent = guidance.detail;
  root.querySelector('[data-tutorial-action]').textContent = guidance.action === 'inspect' ? 'Next: inspect lifecycle evidence'
    : guidance.action === 'wake' ? 'Next: wake the retained Body'
      : guidance.action === 'birth' ? 'Next: choose Forms and birth the Body'
        : 'Next: use an ordinary resident Form';
  return guidance;
}

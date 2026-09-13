export const TOUR_APPLICATION_ACTIONS = Object.freeze({
  schema: "conduit.tour/application-actions@1",
  actions: Object.freeze([
    Object.freeze({ id: "tour.run", scope: "form-proposal" }),
    Object.freeze({ id: "tour.stop", scope: "play-cancellation" }),
    Object.freeze({ id: "tour.restore", scope: "authored-source" }),
  ]),
});

/** Presentation forwards an admitted action identity; it does not interpret
 * lifecycle state or manufacture the resulting Plan, Play, Sign, or evidence. */
const ACTION_CODES = Object.freeze(new Map([
  ["tour.run", 3],
  ["tour.stop", 4],
  ["tour.restore", 5],
]));

export function createTourApplicationActionForwarder({ runtime, onRun, onStop, onRestore }) {
  const handlers = new Map([
    ["tour.run", onRun],
    ["tour.stop", onStop],
    ["tour.restore", onRestore],
  ]);
  return Object.freeze({
    forward(event) {
      const handler = handlers.get(event.action);
      if (!handler) throw new Error("Tour presentation forwarded an unadmitted application action");
      if (runtime.conduit_tour_application_apply(ACTION_CODES.get(event.action)) < 0) {
        throw new Error("shared Tour application refused the lifecycle transition");
      }
      handler();
    },
  });
}

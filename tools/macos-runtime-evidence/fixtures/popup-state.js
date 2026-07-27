(function (root, createReducer) {
  var reducer = createReducer();
  root.PopupCycleReducer = reducer;
  if (typeof module !== "undefined" && module.exports) module.exports = reducer;
})(typeof globalThis !== "undefined" ? globalThis : this, function () {
  function initialState() {
    return { cycle: 0, phase: "ready" };
  }

  function transition(state, input) {
    if (input === "begin" && state.phase === "ready" && state.cycle < 100) {
      var cycle = state.cycle + 1;
      return {
        state: { cycle: cycle, phase: "started" },
        event: { cycle: cycle, phase: "started" },
        action: "show-popup",
      };
    }
    if (input === "shown" && state.phase === "started") {
      return {
        state: { cycle: state.cycle, phase: "shown" },
        event: { cycle: state.cycle, phase: "shown" },
        action: "close-popup",
      };
    }
    if (input === "closed" && state.phase === "shown") {
      var complete = state.cycle === 100;
      return {
        state: {
          cycle: state.cycle,
          phase: complete ? "complete" : "ready",
        },
        event: { cycle: state.cycle, phase: "closed" },
        action: complete ? "close-controller" : "next-cycle",
      };
    }
    throw new Error("invalid popup cycle transition");
  }

  return { initialState: initialState, transition: transition };
});

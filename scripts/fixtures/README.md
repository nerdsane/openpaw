# Hindcast fixtures

Each directory holds a frozen `corpus.md` (nothing dated after the vantage)
and `actuals.json` — the human-authored ground truth the engine is graded
against.

Two properties of `actuals.json` are load-bearing:

1. **Entry order matters.** grade_hindcast matches actuals to forecasts in
   file order, first-substring-match, skipping already-graded forecasts.
   When one question subsumes another (compound vs pure), put the narrower
   or compound-resolving entry FIRST.
2. **Needles should be narrow.** A needle that fails to match leaves a
   forecast ungraded (honest coverage loss); a needle that matches the
   wrong forecast poisons the calibration. Prefer distinctive multi-word
   substrings over single product names.

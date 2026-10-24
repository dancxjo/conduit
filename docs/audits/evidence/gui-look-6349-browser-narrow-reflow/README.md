# Narrow browser graph reflow — #6349

At 390×844 with actual 32px root text, the retained baseline fails: fixed shell bands leave only 22px for the graph. Final production Source `422566f58b136c0ea76ca2ed0b7e9758291acef0` uses narrow document flow and gives the graph 640 visible pixels, with no horizontal document overflow and three successful graph hit samples. Native shell controls remain scrollable.

Explicit Arrange uses current rendered card dimensions divided by actual graph zoom; measured layer widths and card heights avoid overlap. Actual faceplate wrappers equal their visible card bounds. Existing saved positions remain authoritative until the user chooses Arrange; focused reconciliation/retention proof passes. Invalid measurements refuse before replacing the scene. The actual-module Node probe retains the failing pre-change and passing post-change logs.

Final Chromium run: ten Look/furniture cases pass, one worker, zero retries. A separate exact-identity retention case passes. Both package resource cases pass. Final documentary server binary SHA-256: `01bd73df5e0c8ef821aa06740591c674460dde3783e80d0654f203ec6fbeb8ec`. Drivers and source/binary receipts are retained. Navy detail capture uses seven real Zoom in actions; graph zoom still affects physical glyph size and can pan cards off-screen. Full Face remains equal across the capture.

These are local documentary-browser receipts, not live Body, native acceptance, CI, release, publication or parent closure. Full native journey and broader #5366/#6174 work remain open. Complete Look receipts are losslessly gzipped; artifact index records raw/archive hashes. Original full-page and viewport PNGs are retained.

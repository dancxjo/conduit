# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: sdk-external-body-execution.spec.mjs >> a Workspace Body continues through the browser SDK without a Patchbay page runtime
- Location: sdk-external-body-execution.spec.mjs:15:1

# Error details

```
Test timeout of 20000ms exceeded.
```

# Page snapshot

```yaml
- main [ref=f1e2]:
  - heading "Browser SDK Body participation" [level=1] [ref=f1e3]
  - status [ref=f1e4]: Browser Host participating
  - region "Browser Host" [ref=f1e5]:
    - generic [ref=f1e6]:
      - heading "Conduit Browser Host" [level=2] [ref=f1e7]
      - generic [ref=f1e8]:
        - term [ref=f1e9]: Host
        - definition [ref=f1e10]: browser-ephemeral/bddfbd95-e80c-4796-a01f-13a0db7f2d39
        - term [ref=f1e11]: Boot
        - definition [ref=f1e12]: browser-boot/523baf21-cb0e-4105-84f3-690b90e1ea76
        - term [ref=f1e13]: Profile
        - definition [ref=f1e14]: sha256:4444444444444444444444444444444444444444444444444444444444444444
      - paragraph [ref=f1e15]: 3 exact Browser implementation offer(s) admitted.
  - region "External Body Play" [ref=f1e16]:
    - group "External Body Play input" [active] [ref=f1e17]: Body Play input
    - generic [ref=f1e18]:
      - status "button_across_room/indicator" [ref=f1e19]: "false"
      - status "clock-demo/inline-ced6293e47da-0" [ref=f1e20]: "17"
      - status "desk_telegraph/show"
```
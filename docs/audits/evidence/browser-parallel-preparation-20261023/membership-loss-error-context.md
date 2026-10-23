# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: workspace-membership.spec.mjs >> loss of a joined Host Line removes only its current offers and keeps the body
- Location: proof/browser/workspace-membership.spec.mjs:573:1

# Error details

```
Test timeout of 20000ms exceeded.
```

# Page snapshot

```yaml
- generic [active] [ref=e1]:
  - banner [ref=e2]:
    - link "Conduit home" [ref=e3] [cursor=pointer]:
      - /url: /conduit/
      - text: Conduit
    - navigation "Main navigation" [ref=e4]:
      - link "Home" [ref=e5] [cursor=pointer]:
        - /url: /conduit/
      - link "Journeys" [ref=e6] [cursor=pointer]:
        - /url: /conduit/journeys/
      - link "Handbook" [ref=e7] [cursor=pointer]:
        - /url: /conduit/handbook/
      - link "Downloads" [ref=e8] [cursor=pointer]:
        - /url: /conduit/#get-conduit
      - link "Open Conduit" [ref=e9] [cursor=pointer]:
        - /url: /conduit/workspace/
    - link "Source" [ref=e10] [cursor=pointer]:
      - /url: https://github.com/dancxjo/conduit
  - link "Skip to the plot" [ref=e11] [cursor=pointer]:
    - /url: "#plot-input"
  - generic [ref=e12]:
    - banner [ref=e13]:
      - link "Conduit home" [ref=e14] [cursor=pointer]:
        - /url: ../
        - text: c. conduit
      - generic [ref=e15]:
        - heading "Selamawit Sara Rahel" [level=1] [ref=e16]
        - text: lulled
        - generic [ref=e17]:
          - button "parts / hosts" [expanded] [ref=e18] [cursor=pointer]
          - button "Invite another host" [ref=e19] [cursor=pointer]
    - group [ref=e20]:
      - generic "Learn through this body" [ref=e21] [cursor=pointer]
      - generic [ref=e23]:
        - text: Wake this body
        - status [ref=e24]: Tutorial · wake — Birth made one retained body. Wake admits its first exact plan and Play without creating another body.
        - button "Wake the retained body" [ref=e25] [cursor=pointer]
        - status [ref=e26]: "Lifecycle evidence — Not yet fulfilled. Additional lifecycle evidence remains: Wake through an admitted plan and Play; Establish an exact current plan; Start ordinary plot work; Repair a real failed Wake. You can keep exploring."
    - main [ref=e27]:
      - region [ref=e28]:
        - generic [ref=e29]:
          - heading "parts and hosts" [level=2] [ref=e30]
          - button "back to the surface" [ref=e31] [cursor=pointer]
        - generic [ref=e32]:
          - paragraph [ref=e33]: Selamawit Sara Rahel has 2 retained Parts. Presence is current observation, not authority.
          - generic [ref=e34]:
            - article [ref=e35]:
              - heading "This browser" [level=3] [ref=e36]
              - paragraph [ref=e37]: admitted · present · local host
              - group [ref=e38]:
                - generic "Exact membership evidence" [ref=e39]
            - article [ref=e40]:
              - heading "Body Part" [level=3] [ref=e41]
              - paragraph [ref=e42]: admitted · offline
              - group [ref=e43]:
                - generic "Exact membership evidence" [ref=e44]
          - generic [ref=e45]:
            - button "Add a host" [ref=e46] [cursor=pointer]
            - button "Invite another host" [ref=e47] [cursor=pointer]
            - button "Connect a running host" [ref=e48] [cursor=pointer]
    - status [ref=e49]: The joined Host Line was lost. Voice stopped; Body Chat remains available when its model capability is present.
```
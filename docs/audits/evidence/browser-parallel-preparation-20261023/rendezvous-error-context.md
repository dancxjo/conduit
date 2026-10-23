# Instructions

- Following Playwright test failed.
- Explain why, be concise, respect Playwright best practices.
- Provide a snippet of code with the fix, if possible.

# Test info

- Name: creche-rendezvous.spec.mjs >> a rendezvous code admits one already-running raw Host through the web Crèche
- Location: proof/browser/creche-rendezvous.spec.mjs:19:1

# Error details

```
Test timeout of 20000ms exceeded.
```

# Page snapshot

```yaml
- generic [ref=e1]:
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
        - heading "Francisco Guerreiro Henriques" [level=1] [ref=e16]
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
        - status [ref=e26]: "Lifecycle evidence — Not yet fulfilled. Additional lifecycle evidence remains: Wake through an admitted plan and Play; Establish an exact current plan; Start ordinary plot work; Repair a real failed Wake; Admit another host. You can keep exploring."
    - main [ref=e27]:
      - region [ref=e28]:
        - generic [ref=e29]:
          - heading "parts and hosts" [active] [level=2] [ref=e30]
          - button "back to the surface" [ref=e31] [cursor=pointer]
        - generic [ref=e32]:
          - paragraph [ref=e33]: Francisco Guerreiro Henriques has 1 retained Part. Presence is current observation, not authority.
          - article [ref=e35]:
            - heading "This browser" [level=3] [ref=e36]
            - paragraph [ref=e37]: admitted · present · local host
            - group [ref=e38]:
              - generic "Exact membership evidence" [ref=e39]
          - generic [ref=e40]:
            - button "Add a host" [ref=e41] [cursor=pointer]
            - button "Invite another host" [ref=e42] [cursor=pointer]
            - button "Connect a running host" [ref=e43] [cursor=pointer]
    - status [ref=e44]: 3 Plots in Francisco Guerreiro Henriques.
```
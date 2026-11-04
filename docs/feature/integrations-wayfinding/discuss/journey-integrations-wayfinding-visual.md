# Journey — find the integrations and learn what's missing

Persona: Priya, the workspace admin. On a small instance she is also the
instance operator and the only user. Lena, a team lead who is not an admin,
is the secondary actor.

Emotional arc: Confused → Oriented → Confident.

```
 Board (ABC)            Project integrations            New Slack route
 ───────────            ────────────────────            ───────────────
 sidebar:               "Routes here use the            Instance not set up:
  Home  Board            workspace's Slack               "Slack is not set up on this
  …                      connection to Canzan Labs."      Foundry yet. An instance
  Integrations ◄─W-1     [Manage workspace                operator adds the Slack app once."
                          integrations] ◄─W-6 (admin)     [How to set up Slack] ─► /workspace/integrations/slack/setup
 [Integrations] ───────► or, nothing connected:          Workspace not connected (admin):
  (v0.18.1)              "This workspace has no chat      "This workspace has no Slack
                          connection yet."                 connection yet."
                         [Connect Slack or Matrix in      [Connect Slack in workspace
                          workspace integrations] ◄─W-7    integrations] ─► /workspace/integrations
                         [New route] ─────────────────►  Lead (not admin): K-4, unchanged

 Feels: "Where do I       Feels: "Oh, it reuses the       Feels: "Right, the server needs
 set this up?"            workspace's connection;         the Slack app first. Here's how."
                          I just pick a channel."
```

The ending the user hit on 2026-11-02, for contrast:

```
 New Slack route: "This workspace has no chat connection yet. Ask a workspace
 admin to connect Slack or Matrix."   (no link; the reader is the admin; the
 real cause is the instance)          Feels: "Something is wrong with my account."
```

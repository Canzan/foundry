# Journey: set up integrations for this Foundry from the browser

Persona: Priya Raman, instance admin (`instance_admins` row) and workspace
admin of Canzan Labs. Secondary: Tomás Rivera (workspace admin only), Lena Park
(lead, not an admin), and Marco (the access foil). The example instance is
`https://foundry.example.org`.

Emotional arc (Problem Relief): Stuck → Guided → Relieved.

- **Stuck:** "Slack is not set up on this Foundry yet." and "Matrix is not set
  up…". Today the only fix is editing Foundry's environment and restarting.
- **Guided:** one page says what this Foundry has and lacks. One button makes
  the secrets key, which makes Matrix ready. The page shows what to paste into
  Slack and asks for three values.
- **Relieved:** Add to Slack and Connect Matrix appear on the workspace page,
  with no restart.

Two keys:

- The **instance key** (operator-set, or a file Foundry makes) never lives in
  the database.
- The **secrets key** (made in the UI) is stored encrypted under it, and seals
  every credential.

Copy ids I-n are in user-stories.md.

```
 Workspace integrations        Instance integrations (/admin/instance/integrations)                Workspace integrations
 ──────────────────────        ───────────────────────────────────────────────────                 ──────────────────────
 Slack is not set up on this   Instance key  set by the operator  (I-25)
 Foundry yet. ... (W-2)        Secrets key   This Foundry has no secrets key yet ... (I-3)
 Matrix is not set up on this                [Generate secrets key] ──► I-21 "Secrets key        Matrix  [Connect Matrix]
 Foundry yet. ... (W-2)                       generated. Matrix is ready ..."
 [How to set up Slack]         Matrix        ready (I-8)
 [Set up integrations for      Slack         not set up (I-5)
  this Foundry] ◄─ I-19 (B) ─► (I-29 first if FOUNDRY_PUBLIC_URL is unset)
 (instance admins only)          1. Create the app in Slack from ${manifest}
                                    Redirect URL ${slack_return_url}  Events URL ${slack_events_url}
 or: sidebar Instance admin      2. Client ID [6123456789012.7234567890123]
     › Integrations (I-1)           Client secret [••••]  Signing secret [••••]
                                    [Save Slack app] ──► I-10                                     Slack   [Add to Slack]
                               Activity  5 Nov 2026 · Priya Raman generated the secrets key
                                         5 Nov 2026 · Priya Raman saved the Slack app (...)

 Feels: stuck                  Feels: guided ("one button, then three values")                     Feels: relieved
```

## Branches

```
 Standalone, no key setting            Upgraded instance                       Lost / different instance key
 ──────────────────────────            ─────────────────                       ─────────────────────────────
 Instance key: made by Foundry in      Saved credentials are sealed directly   This Foundry cannot open its secrets
 /var/lib/foundry/instance.key. Back   with the instance key ... (I-2e)        key: the instance key it was sealed
 up this file ... (I-25)               [Move to a secrets key] ──► I-22        with is missing or different on this
 (cannot write it: I-27, board up)     Slack set in environment (I-7):         server ... Restore it (A: I-26)
 operator key replaces file: I-30        save same app ──► I-17                [Start over] (B) ──► I-28 cost ──► confirm
 Feels: reassured, knows what to       Feels: cautious → assured (nothing      Feels: alarmed → informed (never
 back up                               broke during the upgrade)               silent; knows both ways out)

 Deliverable B. Replace the secrets key (withheld under I-26): I-23 "Foundry seals its 3 saved credentials again ... Nobody needs to reconnect." ──► I-24
 Remove / switch the Slack app: I-15 / I-14 with the count of connected workspaces, before anything changes
```

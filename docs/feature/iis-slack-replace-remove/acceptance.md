# Acceptance brief: iis\-slack\-replace\-remove

## Slices

None.

## Expectations

### Rows

#### E1

##### Promises

P1

```text
Opening the Slack replace form shows the saved app's client ID already filled in, so Priya does not have to retype it.
```

#### E2

##### Promises

P1

```text
Submitting the replace form with the saved app's own client ID and the secret field left blank keeps the secret already saved: nothing is cleared and Slack is not left without a secret.
```

#### E3

##### Promises

P1

```text
A replace that keeps the same client ID is saved straight away, with no confirmation page or extra confirming step in between.
```

#### E4

##### Promises

P1

```text
Within 60 seconds of such a save, every replica accepts Slack's events using the signing secret now in effect, so Slack keeps working whichever replica answers.
```

#### E5

##### Promises

P1

```text
After that save Priya reads "Saved. Workspace admins can now choose Add to Slack."
```

#### E6

##### Promises

P2

```text
Switching to a client ID different from the current app's is accepted only when Priya supplies both secrets.
```

#### E7

##### Promises

P2

```text
If either secret is left blank with a different client ID, Priya reads "Enter the {field} from the Slack app's Basic Information." naming the field she left blank, and the Slack app in use is unchanged.
```

#### E8

##### Promises

P3

```text
While at least one workspace is connected to Slack, submitting a different client ID takes Priya to a separate confirmation page instead of saving.
```

#### E9

##### Promises

P3

```text
The confirmation page tells Priya how many workspaces are connected, worded in the singular when exactly one is connected, and names the client ID of the app currently in effect.
```

#### E10

##### Promises

P3

```text
Nothing changes while that confirmation is pending: Slack keeps working with the current app until Priya chooses "Save the new app".
```

#### E11

##### Promises

P3

```text
The confirmation page shows neither of the secrets Priya just submitted.
```

#### E12

##### Promises

P3

```text
The submitted app is held for only 10 minutes: confirming after that does not save the new app.
```

#### E13

##### Promises

P3

```text
Only the same instance admin who submitted the change can complete the confirmation; a confirmation sent by anyone else does not save the app.
```

#### E14

##### Promises

P3

```text
The same confirmation step guards replacing a Slack app that came from the environment, not only one saved through the admin UI.
```

#### E15

##### Promises

P4

```text
Opening the remove page only shows information and removes nothing, including when a browser or link preview prefetches it.
```

#### E16

##### Promises

P4

```text
The remove page tells Priya how many workspaces are connected to Slack, with its own wording for the case where no workspace is connected.
```

#### E17

##### Promises

P4

```text
Submitting the removal removes the saved Slack app and Priya reads "Removed. Slack is not set up on this Foundry."
```

#### E18

##### Promises

P5

```text
After removal a workspace admin opening the workspace integrations page sees Slack presented in its W-2 state rather than as an app available to add.
```

#### E19

##### Promises

P5

```text
After removal Slack's event callbacks are refused with 401, so the removed app can no longer deliver events.
```

#### E20

##### Promises

P5

```text
After removal the Slack connection states already shown on the workspace integrations page are not marked lost or broken, and no workspace admin is emailed about the removal.
```

#### E21

##### Promises

P5

```text
Slack deliveries already pending when the app is removed keep waiting on the route history instead of failing at once, still bounded by the 24-hour give-up.
```

#### E22

##### Promises

P6

```text
After a replace, the instance integrations Activity list shows a dated line naming the person who replaced the Slack app and the client ID involved.
```

#### E23

##### Promises

P6

```text
After a removal, the instance integrations Activity list shows a dated line naming the person who removed the Slack app and the client ID involved.
```

#### E24

##### Promises

P7

```text
While the environment supplies the Slack app in effect, the instance integrations page offers Priya neither Replace nor Remove.
```

#### E25

##### Promises

P7

```text
In that state a removal Priya submits anyway changes nothing and answers with the integrations page rather than removing the app or showing a dead end.
```

## Charter

None.

## Examination

None.

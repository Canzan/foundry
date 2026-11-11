# Acceptance brief: iis\-slack\-replace\-remove

## Slices

### S1

##### Covers

P1

```text
An instance admin whose Foundry already holds a saved Slack app in effect, with no workspace connected to Slack, opens the instance integrations page.
```

```text
The page offers the Slack app form with that app's client ID already filled in and both secret fields empty, and shows no key and no secret anywhere.
```

```text
Priya can replace a secret without retyping the client ID, and the page hands no secret back to her.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The replace form offers the saved app\'s client ID and asks for no secret Priya keeps

```text
A form that pre-fills the client ID while its door ignores a blank secret field would still pass this row; the rotation rows observe what a blank field keeps.
```

###### Obligations


##### Covers

P1

P6

```text
With that app saved and in effect and no workspace connected, she submits the same client ID, a blank client secret and the signing secret Slack has just regenerated.
```

```text
She reads that it is saved and that workspace admins can now choose Add to Slack, on the integrations page itself with no confirmation step in between; the page still names the same app with both secrets set, hands back the client ID with both secret fields empty, and shows neither key nor secret - the secret she has just submitted included; and the Activity list reads, newest first, that she replaced that Slack app, then that she saved it, then that she generated the secrets key, and nothing else.
```

```text
Priya rotates one secret in the browser, is told it is done, reads the form back empty of secrets, and the record names her as having replaced that app.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Rotating the signing secret\, with the client secret left blank\, is saved at once

```text
A save that stored the regenerated signing secret but silently re-entered the old one would still pass this row; the rows below observe which secret Slack events verify with.
```

###### Obligations



##### Covers

P1

```text
On a Foundry running as two replicas, she turns Matrix on, enters the Slack app, and then rotates its signing secret, with nothing restarted.
```

```text
Within a minute, and with no restart, each replica asked directly verifies a Slack event signed with the signing secret now in effect.
```

```text
Slack keeps working whichever server answers it, right after Priya rotates the secret.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin turns on Matrix and Slack and rotates a Slack secret\, and every replica follows without a restart

```text
A change that reached only the replica that took it would be caught, but a rotation that took minutes to arrive would still pass if the wait were read per replica rather than for the whole observation.
```

###### Obligations


##### Covers

P1

```text
After that rotation, Slack sends its address-verification message signed with the secret it regenerated.
```

```text
Foundry verifies it, and Slack confirmation of the events address succeeds.
```

```text
Slack events reach Foundry again with the new secret, without anyone restarting anything.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: After the rotation Slack\'s events verify with the signing secret now in effect

```text
A Foundry that accepted every message, signed or not, would still pass this row; the next row observes a message signed with the secret the rotation replaced.
```

###### Obligations


##### Covers

P1

```text
After that rotation, Slack sends its address-verification message signed with the signing secret the rotation replaced.
```

```text
Foundry refuses it as unauthorised.
```

```text
A leaked secret stops working the moment Priya rotates it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The signing secret the rotation replaced no longer verifies

```text
A Foundry that refused every message would still pass this row; the row above observes the secret now in effect being accepted.
```

###### Obligations


##### Covers

P1

```text
After that rotation, a workspace admin chooses Add to Slack and approves Foundry in Slack.
```

```text
The workspace is connected and its integrations page says so, because Slack exchanged the code with the client secret the blank field kept.
```

```text
Rotating one secret does not break Add to Slack: the client secret Priya left blank is still the one Slack checks.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Add to Slack still exchanges Slack\'s code with the client secret the rotation kept

```text
A blank field that stored an empty client secret would still pass if Slack accepted any secret; the Slack this scenario talks to accepts only the client secret the app was entered with.
```

###### Obligations



##### Covers

P2

```text
With one app saved and in effect, she submits a different client ID with the client secret left blank, and in a second run with the signing secret left blank.
```

```text
She reads that she must enter that field from the Slack app Basic Information page, the form hands back the client ID she typed with both secret fields empty, and the app already in effect is untouched: the page still names it with both secrets set, no replacement is recorded, Add to Slack still asks Slack for it, and its events still verify.
```

```text
Switching apps cannot half happen: Priya is asked for the secret she left out, and nothing about the app that works moves.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Another Slack app needs both secrets\, and a blank one changes nothing

```text
A refusal that stored the submitted secret somewhere out of sight would still pass this row; what a page shows is read by the rows that sweep the page for secrets.
```

###### Obligations


##### Covers

P2

```text
With one app saved and in effect and no workspace connected to Slack, she submits a different client ID with both of its secrets.
```

```text
She reads that it is saved and that workspace admins can now choose Add to Slack, on the integrations page with no confirmation step in between; the page names the new app, and Add to Slack asks Slack approval for the new app.
```

```text
While nobody is connected, Priya moves this Foundry to another Slack app in one step.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Another Slack app entered with both secrets takes over at once while no workspace is connected

```text
A save that sealed the old app secrets under the new client ID would still pass this row; the blank-field rows above refuse the submission that would do it, and the workspace connection is observed only for the app whose secrets were kept.
```

###### Obligations



##### Covers

P2

```text
Any of the three fields is submitted with surrounding spaces, in upper case, with a character outside its alphabet, with two dots or with none, on the replace path as on the first entry.
```

```text
The verdict is the one the field shapes already give: spaces around a value do not change it, any 32 lowercase hexadecimal value is a secret, and a value outside a field alphabet is named as malformed rather than as missing.
```

```text
Priya gets the same answer about what she typed whether she is entering Slack for the first time or replacing it, so a rotation never fails for a reason she has not been told.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs \:\: the field\-shape table and the generated field cases of the decision module

```text
A refactoring that kept every field verdict but stopped trimming the client ID before comparing it with the app in effect would still pass these cases; which app a submission names is observed by the rows above.
```

###### Obligations


##### Covers

P1

```text
On a Foundry the operator runs as its own process, she generates the secrets key, enters the Slack app, rotates its signing secret, connects a workspace, and then everything this Foundry shows, logs and stores is searched for every key and secret it was ever handed, the replaced one included.
```

```text
None of them appears in any page, page source, log line or activity line, and none is stored in plaintext.
```

```text
The secret Priya pasted in to replace the old one is kept as closely as the first one she entered: nothing hands it back to a reader, writes it to a log or stores it in the clear.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: No key or secret appears in any page\, log line or activity line\, or in plaintext in the database

```text
A Foundry that leaked the replaced secret only on a page this sweep does not open would still pass; the sweep reads the pages Priya and a workspace admin walk through, plus the whole database and the process output.
```

###### Obligations

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

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

### S2

##### Covers

P3

```text
One workspace is connected to Slack through the Slack app this Foundry has in effect, and the instance admin submits a different Slack app with both of its secrets.
```

```text
Nothing is switched yet: she is taken to a page of its own that tells her one workspace is connected through the current app, names that app, says it keeps posting through it until a workspace admin reconnects Slack, and asks whether to save the new app, with one control to do it and neither secret she just typed anywhere on it. The app in effect is untouched - the page still names it with both secrets set, nothing is recorded as replaced, Add to Slack still asks Slack for it, and its events still verify.
```

```text
Priya learns how many workspaces her switch costs before it happens, and can still walk away leaving Slack exactly as it was.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Another Slack app waits while a workspace is connected through the current one

```text
A hold that bumped this Foundry's configuration version while changing nothing a reader can see would still pass this row, and so would a count that read the number of workspaces rather than the workspaces connected through Slack: one workspace exists here. The page's own table of the count forms reads the other counts.
```

###### Obligations



##### Covers

P3

P6

```text
With that submission waiting, the same instance admin opens the page twice without choosing anything, and two minutes later chooses to save the new app.
```

```text
She reads that it is saved and that workspace admins can now choose Add to Slack, on the integrations page itself; the page names the new app, shows no key and no secret, Add to Slack now asks Slack approval for the new app, and the Activity list reads, newest first, that she replaced the Slack app naming the new one, then that she saved the first one, then that she generated the secrets key, and nothing else.
```

```text
Priya switches apps in one deliberate step, reads the record of what she did, and workspace admins are sent to the new app from then on.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The admin who submitted the other app saves it from the confirm page

```text
An apply that re-read the submitted fields from the request instead of the app held sealed would still pass this row; the expired-submission row is what observes that the held one is the source.
```

###### Obligations


##### Covers

P3

```text
With that submission waiting, a second instance admin of this Foundry confirms it instead.
```

```text
His confirm decides nothing: he is answered with this Foundry's integrations page, and the app in effect is untouched - the page still names it with both secrets set, nothing is recorded as replaced, Add to Slack still asks Slack for it, and its events still verify.
```

```text
A switch Priya started cannot be completed by someone else who happens to open the page.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Another instance admin\'s confirm saves nothing

```text
A confirm door that turned away every instance admin, Priya included, would still pass this row; the row below saves through that same door after his attempt.
```

###### Obligations


##### Covers

P3

```text
After that second instance admin's confirm, the admin who submitted the app chooses to save the new app.
```

```text
Her own confirm still saves it: she reads that it is saved and that workspace admins can now choose Add to Slack, and the page names the new app.
```

```text
Someone else's attempt does not throw Priya's waiting submission away: she still only has to confirm it once.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: After another instance admin\'s confirm the submitting admin still saves the new app

```text
A confirm that ignored who submitted the app would still pass this row; the row above is what turns the other admin away.
```

###### Obligations


##### Covers

P3

```text
With that submission waiting, eleven minutes pass before the same instance admin confirms it.
```

```text
Her confirm decides nothing: she is answered with this Foundry's integrations page, and the app in effect is untouched - the page still names it with both secrets set, nothing is recorded as replaced, Add to Slack still asks Slack for it, and its events still verify.
```

```text
A switch left half-finished does not go through by itself much later; Priya enters it again if she still wants it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A confirm eleven minutes after the submission saves nothing

```text
A submission that expired the instant it was made would still pass this row; the saving rows confirm inside the ten minutes it is held.
```

###### Obligations


##### Covers

P3

```text
After the new app is saved from that page, the same confirm is sent once more, as a stale tab or a reload would send it.
```

```text
Nothing is saved a second time: she is answered with this Foundry's integrations page, which names the new app, and the Activity list still reads exactly the replacement, the first save and the secrets key, newest first.
```

```text
A reload or a forgotten tab cannot record Priya's switch twice or re-run it behind her.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A second confirm does not save the new app twice

```text
An implementation that left the submission stored but refused to apply it twice would still pass this row: nothing a reader sees tells a discarded submission from an unusable one.
```

###### Obligations


##### Covers

P3

```text
With a Slack app in effect, a workspace connected through it and nothing submitted, the instance admin sends a confirm anyway.
```

```text
It decides nothing: she is answered with this Foundry's integrations page, and the app in effect is untouched - the page still names it with both secrets set, nothing is recorded as replaced, Add to Slack still asks Slack for it, and its events still verify.
```

```text
Reaching the confirm address with nothing waiting tells Priya where she is instead of changing Slack or showing an error.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A confirm with nothing submitted changes nothing

```text
A door that answered the page for every confirm would still pass this row; the saving rows observe a confirm that does take effect.
```

###### Obligations


##### Covers

P3

```text
The Slack app in effect is the one Foundry's environment sets, one workspace is connected through it, and the instance admin submits a different Slack app with both of its secrets.
```

```text
The same wait applies: she is taken to the page that tells her one workspace is connected through the current app, naming the app the environment set, with one control to save the new app and neither secret on it, and nothing is recorded - the Activity list still reads only that she generated the secrets key.
```

```text
An operator-set Slack app is not switched out from under connected workspaces without Priya reading the cost first.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The app Foundry\'s environment sets also waits for the confirm

```text
A gate that compared the submission with a saved app rather than with the app in effect would still pass this row, because no app is saved here; the row above, where the current app is the saved one, is what reads the app in effect.
```

###### Obligations


##### Covers

P3

```text
With that submission waiting while the environment sets the app in effect, the same instance admin chooses to save the new app.
```

```text
It is stored: she reads that it is saved and that the app in Foundry's environment stays in effect until those three settings are removed and Foundry restarts, she is on the integrations page, the Activity list names the app she stored, and Add to Slack still asks Slack approval for the environment's app.
```

```text
Priya can prepare the app that takes over when the operator drops the environment settings, and until then nothing about Slack changes.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The confirm stores the other app while the environment\'s stays in effect

```text
A store that kept the new app's client ID and dropped both of its secrets would still pass this row: nothing reads them while the environment's app is the one in effect.
```

###### Obligations


##### Covers

P3

```text
Two or three workspaces, or none, are connected through the Slack app in effect when a different app is submitted.
```

```text
With none connected the app is saved at once with no page in between; with two or three the page reads the counted wording naming how many are connected, never the wording that fits exactly one.
```

```text
The number Priya reads is the number of workspaces that keep posting through the old app, and she is not stopped when nobody is connected.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs \:\: the table of the count forms I\-14 and I\-15 read\, and of the gate the connected count decides

```text
A page that read the counted wording from a number nobody counted would still pass that table; the connected count itself is the store's own read, and the saved-at-once case is read through the public page.
```

###### Obligations





##### Covers

P1

P3

```text
One workspace is connected to Slack through the app in effect, and the instance admin replaces that same app's signing secret, leaving the client secret blank.
```

```text
It is saved straight away with nothing in between: she reads that it is saved and that workspace admins can now choose Add to Slack, on the integrations page, which still names that same app, and nothing is left waiting for a confirmation she was never asked for.
```

```text
Rotating a leaked or regenerated secret stays a one-step job for Priya even once workspaces are connected: she is only warned where a switch would actually cost them their connection.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A rotation of the current app\'s own secret is saved at once while a workspace is connected

```text
A gate that read whether the app in effect comes from the environment, rather than which app the submission names, would still pass this row; the environment rows are what read that case.
```

###### Obligations


##### Covers

P3

```text
The confirmation of a waiting Slack app arrives exactly at the instant the ten minutes it is held run out.
```

```text
It is not applied and the submission is gone: the instant the hold ends is already too late, as every later instant is.
```

```text
The ten minutes Priya has to confirm a switch end where she was told they end, not an instant after.
```

##### Disposition

Tested

crates\/foundry\-store\/src\/integrations\/instance\.rs \:\: the finite table of when a held Slack app may still be applied\, run by the iis\_instance\_store target

```text
A table that set its clock a whole minute past the hold would still pass while leaving the fencepost unread; the instant itself is the case this row names.
```

###### Obligations


##### Covers

P3

```text
The confirm page is opened, with a submission waiting and with none, and the remove page is opened with an app saved.
```

```text
Each of those openings only reads: the stored rows, the recorded activity and this Foundry's configuration version are exactly what they were, so no replica is told to reload anything.
```

```text
A browser or a link preview that opens one of these pages for Priya decides nothing and disturbs no server.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs \:\: the table of what the confirm and remove pages reach when they are only read

```text
A handler that bumped the version through a write the table does not list would still pass it; the public rows observe the rows and the activity, which that write would also move.
```

###### Obligations



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

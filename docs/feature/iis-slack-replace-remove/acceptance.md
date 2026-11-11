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

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs \:\: a\_value\_is\_judged\_the\_same\_with\_surrounding\_whitespace

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
The confirm page is opened, with a submission waiting and with none.
```

```text
Each of those openings only reads: the stored rows, the recorded activity and this Foundry configuration version are exactly what they were, so no replica is told to reload anything.
```

```text
A browser or a link preview that opens the confirm page for Priya decides nothing and disturbs no server.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs \:\: the table of what the confirm and remove pages reach when they are only read

```text
A handler that bumped the version through a write the table does not list would still pass it; the public rows observe the rows and the activity, which that write would also move.
```

###### Obligations



### S3

##### Covers

P7

```text
An instance admin opens this Foundry integrations page with the Slack app she entered here in effect.
```

```text
The page offers her both a way to replace that app and a way to remove it.
```

```text
Switching or retiring Slack stays a job Priya does in the browser, instead of an environment edit and a restart.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The page offers Replace and Remove over the Slack app Priya entered

```text
A page that offered both controls under the app Foundry environment sets as well would still pass this row; the environment rows read that case.
```

###### Obligations


##### Covers

P4

```text
One workspace is connected to Slack through the saved app, a browser has already opened the remove page once, and the instance admin now follows Remove on the integrations page.
```

```text
She lands on the remove page at its own address, which asks whether to remove the Slack app from this Foundry and says that 1 workspace Slack routes stop posting until it is set up again and that no workspace can connect Slack, offers one Remove control and shows no key and no secret; after both of those readings the app is still in effect with both its secrets and its recorded activity exactly as they were.
```

```text
Priya reads what a removal costs the workspaces that depend on it before she decides, and a browser that opens the page for her removes nothing.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Remove says how many workspaces stop posting\, and reading it removes nothing

```text
A remove page that counted every workspace of this Foundry rather than those connected to Slack would still pass this row, where one workspace is connected and one exists; the count forms table reads the larger counts.
```

###### Obligations



##### Covers

P4

```text
No workspace is connected to Slack, and the instance admin follows Remove on the integrations page.
```

```text
The remove page asks whether to remove the Slack app from this Foundry and says only that no workspace is connected to Slack, with one Remove control.
```

```text
Priya is not warned that workspaces stop posting when none of them is connected.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: With nobody connected\, Remove says only that no workspace can connect Slack

```text
A page that always read the no-workspace form would still pass this row on its own; the connected row is what reads the counted form.
```

###### Obligations


##### Covers

P4

P6

```text
With the Slack app saved here and nobody connected to it, the instance admin chooses Remove on the remove page.
```

```text
She reads that it is removed and that Slack is not set up on this Foundry, lands back on the integrations page where no app is set up and no workspace is offered Add to Slack, and the Activity list reads, newest first, that she removed the Slack app naming it, that she saved that app, and that she generated the secrets key, with no other line.
```

```text
The removal takes one step and leaves a record a reader can tell apart from a first save or a replacement.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Removing the Slack app says Slack is gone and records who removed it

```text
A removal that also dropped a workspace Slack connection would still pass this row, which has no connection to lose; the connected rows read what a removal leaves alone.
```

###### Obligations



##### Covers

P5

```text
One workspace is connected to Slack, a route of one of its projects holds a message Slack has not answered yet, and the instance admin removes the Slack app.
```

```text
That workspace integrations page reads that Slack is not set up on this Foundry yet and that an instance operator adds the Foundry Slack app once; its Slack connection is not marked lost and no workspace admin is emailed; and the route still holds the message it had accepted, retrying.
```

```text
Retiring Slack for the whole instance never tells a workspace it lost its own connection, and never throws away a message Foundry had already taken.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: After the removal Canzan Labs reads that Slack is not set up\, and loses nothing it had

```text
A removal that left the route retrying for Slack own silence rather than for the app that is gone would still pass this row; what it refuses is a delivery the removal cancelled or gave up on.
```

###### Obligations



##### Covers

P5

```text
After the removal, with the workspace that was connected still holding its Slack connection and no app in effect on this Foundry, the instance admin enters the same Slack app again with both of its secrets.
```

```text
It is saved at once: she reads that it is saved and that workspace admins can now choose Add to Slack, which a submission held for a confirmation would not let her read, and the connected workspace integrations page reads that it is connected to Canzan Labs without anybody reconnecting.
```

```text
A workspace that was connected before Priya retired Slack is connected again the moment she sets it up, with nothing for its admin to redo and nothing for her to confirm about an app that is gone.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Setting the same app up again finds Canzan Labs still connected

```text
A removal that kept the connection row but forgot its token would still pass this row, which reads the connection line rather than a post through it; a gate that sent this submission to a confirmation naming no app would fail it, while a gate that saved at once whatever is in effect would still pass it, and the gate table reads that.
```

###### Obligations


##### Covers

P5

```text
This Foundry runs as two replicas on one database, each of them holding the Slack app the instance admin entered, and she removes it.
```

```text
Within 60 seconds, with nothing restarted, each replica asked at its own address refuses as unauthorised a Slack event signed with the signing secret the removed app was saved with.
```

```text
Once Priya retires Slack, no server of this Foundry acts on Slack messages any more, whichever one Slack reaches.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Once the Slack app is removed no replica accepts Slack\'s events any more

```text
A Foundry that refused every Slack event as unauthorised even with an app in effect would still pass this row; the replace rows read the verifying case.
```

###### Obligations


##### Covers

P7

```text
All three Slack settings are in Foundry environment, an app is also saved here, and the instance admin opens the remove address.
```

```text
She is answered with this Foundry integrations page, which offers her neither a way to replace nor a way to remove the Slack app, by any name or link.
```

```text
Priya is not offered a change to the Slack app the operator settings decide, which the next restart would undo.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The Slack app Foundry\'s environment sets is not Priya\'s to replace or remove

```text
A door that answered the integrations page for every reader, the saved-app case included, would still pass this row; the saved-app rows read that case.
```

###### Obligations

None.

##### Covers

P7

```text
With all three Slack settings in Foundry environment, an app saved here and a workspace connected, a remove request is posted to the remove address.
```

```text
The answer is this Foundry integrations page, which still reads that the Slack app comes from Foundry environment, the Activity list records no removal, and Add to Slack still sends a workspace admin to that same app.
```

```text
A stale tab or a forged form cannot take away a Slack app that is not this page to give.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A remove posted while Foundry\'s environment sets the Slack app changes nothing

```text
A door that refused every remove, the saved-app case included, would still pass this row; the removal rows read the accepted case.
```

###### Obligations


##### Covers

P7

```text
The controls the integrations page offers are read for each app that can be in effect: the environment one with an app also saved here, the environment one with none saved, the app saved here, and none at all.
```

```text
Replace and Remove are offered only where the app in effect is the one saved here; in every other case neither is offered and a remove changes nothing.
```

```text
Priya is offered a change to this Foundry Slack app exactly where it is hers to make, and nowhere else.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/handlers\/instance\.rs \:\: the table of the controls the page offers over each app in effect

```text
A table read over only the two cases the public rows construct would still pass; the whole set of apps that can be in effect is what this row names.
```

###### Obligations



##### Covers

P2

```text
Each of the three fields of the Slack app form is given any value over its own alphabet or any other text, wrapped in up to four spaces, tabs or newlines.
```

```text
The verdict on the value is the same with the surrounding whitespace as without it, for every generated case.
```

```text
A client ID or a secret Priya pastes with a stray space or a line break is read as the value she pasted.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs\:\:a\_value\_is\_judged\_the\_same\_with\_surrounding\_whitespace

```text
A check that trimmed the value but read a different alphabet would still pass this row; the alphabet rows read that.
```

###### Obligations


##### Covers

P2

```text
Any 32 lowercase hexadecimal characters are entered as the client secret and as the signing secret of a submission.
```

```text
Each field accepts the value and the submission is refused on nothing.
```

```text
Every secret Slack can show Priya on its Basic Information page is one she can paste here.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs \:\: any\_32\_lowercase\_hex\_is\_accepted\_as\_a\_secret

```text
A check that also accepted a 31 or 33 character value would still pass this row; the shape table reads the lengths.
```

###### Obligations


##### Covers

P2

```text
A value of any field holds one character outside that field alphabet, anywhere in it, and is not whitespace.
```

```text
The field refuses the value as malformed, naming that field, and never as blank.
```

```text
Priya is told that what she pasted is not the shape of that field, instead of being told she left it empty.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs \:\: a\_character\_outside\_the\_alphabet\_is\_refused\_as\_malformed

```text
A check that called every refused value blank would fail this row, which is why it is here; a check that named the wrong field would still pass the whitespace row.
```

###### Obligations


##### Covers

P2

```text
The three fields are submitted blank, with an uppercase secret, with a client ID holding two dots and one holding none, and with each malformed shape, in form order.
```

```text
The submission is refused on the first failing field in the order the form reads, blank where the field is empty and malformed where its shape is wrong.
```

```text
Priya is sent back to the first field that needs her attention, with the words that say what is wrong with it.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs \:\: a\_save\_is\_refused\_on\_the\_first\_blank\_or\_malformed\_field\_in\_form\_order

```text
A reading that named the last failing field rather than the first would still pass a table of single-field rows; the mixed rows are what order it.
```

###### Obligations




##### Covers

P1

```text
With an app saved and in effect, the instance admin submits its own client ID, leaves the client secret blank and enters a signing secret this Foundry has never held.
```

```text
The save is accepted at once, she reads that it is saved and that workspace admins can now choose Add to Slack, the page still names that same app with both secrets set, and the Activity records the replacement.
```

```text
Pasting one regenerated secret is all Priya does, and the secret she did not retype is kept.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Rotating the signing secret\, with the client secret left blank\, is saved at once

```text
A save that cleared the kept client secret instead of keeping it would still pass this row, which reads the page; the code exchange row reads the secret itself.
```

###### Obligations


##### Covers

P2

```text
With an app saved and in effect, the instance admin submits a different client ID with one of the two secrets left blank, once for each of them.
```

```text
She reads that she must enter that field from the Slack app Basic Information, the form keeps the client ID she typed with both secret fields empty, and the saved app, its secrets and its activity are unchanged.
```

```text
Another app is never half entered: a secret that belongs to the old app is never sealed to a new one.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Another Slack app needs both secrets\, and a blank one changes nothing

```text
A refusal that named the other blank field would still pass a row reading one example; the two examples name their own field.
```

###### Obligations



##### Covers

P1

```text
With an app saved and in effect and no workspace connected to Slack, the instance admin submits a different client ID with both of its secrets.
```

```text
The new app is saved at once, she reads that it is saved, the page names the new app, and a workspace admin choosing Add to Slack is sent to that new client ID.
```

```text
Switching apps while nobody is connected costs Priya nothing but the form.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Another Slack app entered with both secrets takes over at once while no workspace is connected

```text
A save that stored the new app but left the old one in effect would fail this row only on the Add to Slack reading, which is why that reading is in it.
```

###### Obligations


##### Covers

P1

```text
A submission names the app in effect with both secret fields blank, names it with both secrets given, and names it with one field blank over a saved app whose secrets this Foundry cannot open.
```

```text
Both blank fields keep both saved secrets, both given secrets replace both, and over a saved app that does not open a blank field is refused for that field because there is nothing to keep.
```

```text
A blank field means keep what is already there exactly where there is something to keep, and says so where there is not.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs \:\: the table of which secrets a submission keeps and which it replaces

```text
A plan that kept a secret even where the saved app cannot be opened would still pass the two keeping rows; the unopenable row is what refuses it. This table does not exist yet: writing it is a completion obligation of this slice craft.
```

###### Obligations




##### Covers

P1

```text
After a rotation that replaced the signing secret, Slack sends its verification challenge signed with the secret the rotation replaced.
```

```text
This Foundry refuses the message as unauthorised, while the challenge signed with the secret now in effect is verified.
```

```text
A leaked signing secret stops working the moment Priya replaces it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: The signing secret the rotation replaced no longer verifies

```text
A Foundry that refused every challenge would still pass this row; the verifying row beside it is what tells them apart.
```

###### Obligations


##### Covers

P1

```text
A rotation of the app in effect leaves both secret fields blank, and another replaces both of them with values this Foundry has never held.
```

```text
After the blank rotation both opened secrets are the ones that were in effect before it, so the same events verify and the same code exchange succeeds; after the rotation of both, both opened secrets are the new ones and neither replaced value works afterwards.
```

```text
A rotation changes exactly the secrets Priya typed and nothing else.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/slack\_app\.rs \:\: the table of which secrets a submission keeps and which it replaces

```text
A plan that sealed the submitted blank value as an empty secret would still pass a row that only reads the stored page. This table does not exist yet: writing it is a completion obligation of this slice craft.
```

###### Obligations



##### Covers

P1

```text
This Foundry runs as two replicas on one database, and the instance admin generates the secrets key, enters the Slack app and then rotates its signing secret on one of them.
```

```text
Within 60 seconds of each change, with nothing restarted, each replica asked at its own address offers the workspace what the change allows and verifies Slack events signed with the secret now in effect.
```

```text
Whatever Priya changes here is true of every server of this Foundry a minute later, with no restart and no operator.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin turns on Matrix and Slack and rotates a Slack secret\, and every replica follows without a restart

```text
A Foundry whose replicas reloaded only on a restart would fail this row; a Foundry that reloaded only the app and not the key would still pass its last reading.
```

###### Obligations


##### Covers

P1

```text
A save of the Slack app is attempted with its secrets sealed under no key and under a key that is no longer the sealing key, and then under the sealing key itself.
```

```text
Each of the first two is turned back with nothing at all stored, and the third stores the app, its activity line and the version bump together.
```

```text
A secret sealed with a key this Foundry has moved past is never stored where nothing could open it.
```

##### Disposition

Tested

crates\/foundry\-store\/tests\/iis\_instance\_store\.rs \:\: slack\_save\_fences\_on\_the\_sealing\_key

```text
A door that turned the save back and then never retried under the current key would still pass this row, which reads the store alone; the handler retry is read by the page rows.
```

###### Obligations


##### Covers

P3

```text
A submitted app is held for its confirmation with its secrets sealed under a key that is no longer the sealing key, and a secrets-key move is made while a held app is waiting.
```

```text
The stale hold is turned back with no row written, and the move seals the held app secrets again with every other sealed value, so the confirmation still opens them, while the number of credentials the move reports is unchanged by the held row.
```

```text
An app Priya submitted survives a secrets-key move, and a submission nothing could open is never kept.
```

##### Disposition

Tested

crates\/foundry\-store\/tests\/iis\_instance\_store\.rs \:\: the table of what a superseded sealing key and a secrets\-key move do to a held Slack app

```text
A move that re-sealed the held row but also counted it as a credential would still pass a row that only reads the confirmation. This table does not exist yet: writing it is a completion obligation of this slice craft.
```

###### Obligations




##### Covers

P3

```text
Every crate source of this repository is read for queries that name the instance integration tables and for writes of a sealed column.
```

```text
The tables are named only by the one module that owns them, and every sealed write stands inside a listed fenced writer, the hold of a submitted app included; a crate added later is read too.
```

```text
A second writer of this Foundry Slack tables, or a sealed write with no fence, cannot reach a release.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs \:\: the integrations\-instance\-tables and seal\-fenced\-writers rules\, run by cargo xtask check\-arch

```text
A query assembled at run time escapes a reading over string literals; the store tests read what the writes actually do.
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

# Acceptance brief: iis\-secrets\-key\-replace

## Slices

### S1

##### Covers

P1

P2

P4

P5

P11

```text
This Foundry holds a secrets key that opens, a Slack connection, a Matrix bot connection and the Slack app Priya entered, with the project routed to a Slack channel and a Matrix room. A browser has already opened the replace page once; Priya then chooses Replace secrets key beside the stored key, reads the page behind it, confirms, and a new issue is filed afterwards.
```

```text
The page behind the control reads "Replace the secrets key? Foundry seals its 3 saved credentials again under a new key. Nobody needs to reconnect." and offers Replace secrets key, while the stored key and the whole Activity list are as they were before both readings. The confirmation lands Priya on the instance integrations page reading "Replaced. 3 saved credentials are sealed under the new key.", the Activity list reads exactly the replaced line with its count above the two shipped lines, a different secrets key is stored, no page carries it in any encoding, the saved connection still opens, nothing is marked lost, no workspace admin is emailed, and the next new issue reaches both the Slack channel and the Matrix room.
```

```text
Priya rotates the key of a Foundry she doubts in two clicks, reads how many credentials were sealed again, and nobody reconnects or hears about it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Replacing the secrets key seals every saved credential again and nobody reconnects

```text
A replacement that seals the credentials again but leaves the stored key wrapped under the instance key it already used; the key identity is read through the test seam, which is why a message alone cannot pass, but the wrapping itself is observed only by the store tests.
```

###### Obligations




##### Covers

P1

```text
With a secrets key that opens, Priya opens the replace page while this Foundry holds, in turn, nothing saved, only the Slack app she entered, only one workspace connection, and one connection together with that app.
```

```text
Each reading asks "Replace the secrets key?" with the sentence the count calls for: no saved credentials yet and nothing to seal again for none, "its 1 saved credential again" for either single credential, and "its 2 saved credentials again" for both; Replace secrets key is offered every time, and the stored key and the whole Activity list are unchanged afterwards.
```

```text
Before she decides, Priya reads how many credentials a replacement touches, counted the way the page counts them everywhere else, and reading it never changes anything.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:The replace page says how many saved credentials are sealed again\, and reading it replaces nothing

```text
A page that counts the app Foundry's environment sets as a saved credential as well: the third case holds a connection with the environment's app and expects 1, so an over-count shows there, but a Foundry whose environment sets no app at all is not exercised here.
```

###### Obligations








##### Covers

P2

```text
Priya confirms the replacement on the replace page while this Foundry holds, in turn, nothing saved, only the Slack app she entered, and one workspace connection together with that app.
```

```text
She lands on the instance integrations page, which reads "Replaced. This Foundry has a new secrets key." for none, "Replaced. 1 saved credential is sealed under the new key." for one and "Replaced. 2 saved credentials are sealed under the new key." for two, and a different secrets key is stored in every case.
```

```text
Whatever this Foundry holds, Priya is told in plain words how many credentials are now sealed under the key she just made.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Confirming the replacement says how many credentials are now sealed under the new key

```text
A replacement whose notice is carried in the address instead of waiting once in her session: the words are read on the page she lands on either way, and only the reload case tells the two apart.
```

###### Obligations



##### Covers

P2

```text
After a replacement with two saved credentials, Priya opens the instance integrations page she already landed on again, as a reload does.
```

```text
The page reads that the secrets key is set and says nothing about a replacement, the Activity reads the replaced line with its count exactly once, and the saved connection still opens.
```

```text
A reload tells Priya nothing happened twice: the message she already read does not come back, and no second replacement is claimed or made.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:A reload after the replacement reads the page with no notice and claims no second replacement

```text
A notice taken out of the session on the first reading but written into a second place nobody reloads, which this scenario never opens.
```

###### Obligations



##### Covers

P2

P4

```text
This Foundry runs as two replicas on one database with a saved Slack connection and a routed project; a replace request reaches each replica at the same moment, and a new issue is filed afterwards.
```

```text
Both requests are answered with the page reading that the secrets key is set, one key is stored, a different one than before, the Activity records one replacement and not two, the connection opens on every replica within 60 seconds without a restart, nothing is marked lost, no workspace admin is emailed, and every replica's route history lists the new issue as delivered.
```

```text
Two admins pressing Replace at the same moment leave one new key, one recorded rotation and every server still posting.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Two replace requests at the same moment replace the key once\, and every replica opens every credential

```text
A second request that arrives after the first has committed and is refused by reading the state again rather than by the stored key id: the simultaneous case is what the race makes likely, not certain.
```

###### Obligations




##### Covers

P10

```text
After a replacement with two saved credentials, the operator runs the doctor command that rotates the instance key itself.
```

```text
The command exits reporting that no credential had to be sealed again under the new key, printing no key, and afterwards the saved connection still opens and the page still reads that the secrets key is set.
```

```text
A rotation Priya made in the browser does not take the operator's own key rotation away, and nothing has to be reconnected after it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:After a replacement the operator\'s own key rotation still leaves every credential opening

```text
A rotation that re-wraps the stored key but reports its count from the wrong population: the count is read as the command prints it, while the credentials are observed only through the connection that opens.
```

###### Obligations



##### Covers

P3

```text
A replacement whose re-seal refuses on the first connection credential or on the saved Slack app secrets, and the loss of the stored key id to another admin between the reading and the apply.
```

```text
Nothing is stored, nothing is recorded, the old key stays in effect with every credential still opening, and the page answers the confirm question again with no replaced message anywhere.
```

```text
A replacement that cannot finish leaves Priya exactly where she was, reading the question again, with every credential still opening and nothing claiming otherwise.
```

##### Disposition

NotTested

```text
No public door can make a re-seal refuse inside the write: the refusal is a fault of the sealing closure, not of anything a person can type or post, and the state where the stored key cannot be opened withholds Replace altogether, so the public port cannot construct it. The unchanged state under an injected refusal belongs to the store write verification of this slice's commands; the answer itself, the confirm question read again and no replaced message, belongs to the handler verification of the same commands with a store double that refuses, because a database check cannot observe a response. The losing half of the same law, the stored key id changing under the apply, is constructed publicly by the simultaneous replace requests and is covered by its own row.
```

DELIVER\, both halves as completion obligations of this slice\'s own verification\: the store write run over the instance store for the unchanged state under an injected refusal\, and the handler library run with a refusing store double for the answer and the absent notice

### S2

##### Covers

P6

P7

```text
This Foundry cannot open its stored secrets key, three saved credentials cannot be opened and no running Foundry server opens the key; the instance admin opens the instance integrations page and then the way out it offers.
```

```text
She reads the restore words she already read, with a way to start over beside them, no way to replace the key and nothing claiming another server still opens it. The way out states that the three credentials this Foundry cannot open are discarded, that workspace admins reconnect Slack and re-enter Matrix bot tokens and that the Slack app is entered again, and asks her to type a confirmation first. Reading it discards nothing: the stored key, the Activity and the version every replica follows are as they were, and with the instance key set again the page reads the secrets key as set and her saved Slack connection still opens.
```

```text
She sees a confirmed way forward and what it costs before she chooses, instead of restore words she cannot act on.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:A Foundry that cannot open its secrets key offers a way to start over and states its cost first

```text
A page that offers the way out whenever its secrets key does not open, without judging whether another server still opens it; the live-server scenario is what refuses that.
```

###### Obligations




##### Covers

P6

```text
A second Foundry server on the same database still opens the stored secrets key while this one cannot; the instance admin opens the instance integrations page and then the start over address.
```

```text
The restore words, followed by the sentence that another Foundry server still opens the secrets key so starting over is not offered, with no way to start over and no way to replace the key. The start over address answers this Foundry's integrations page, and the stored key, the Activity and the version every replica follows are as they were.
```

```text
She is not offered a way out that would discard credentials a running server is still serving with, and she reads which of the two states she is in.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:While another Foundry server still opens the secrets key\, starting over is not offered

```text
A page that never offers the way out at all; the stopped-server scenario is what refuses that.
```

###### Obligations




##### Covers

P6

```text
That second Foundry server stops and a minute passes, with this Foundry still unable to open its stored secrets key.
```

```text
The page offers the way out again and says nothing about another server, and the way out states that the two credentials this Foundry cannot open are discarded. The stored key, the Activity and the version every replica follows are as they were.
```

```text
Once no running server still opens the key, the way out she was refused a moment ago is offered.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Once that server has stopped\, starting over is offered with its cost

```text
A gate that asks only whether some server ever said it opens the key, since this case turns on how old that server's last word is.
```

###### Obligations


##### Covers

P6

```text
This Foundry reads no instance key at all, its key file unreadable, while a stored secrets key and saved credentials it cannot open are there; the instance admin opens the instance integrations page and then the start over address.
```

```text
The words naming the key file it cannot read, and none of the words about a stored secrets key that cannot be opened - a different state, with its own words - with no way to start over, no way to replace, and nothing claiming another server still opens the key. The start over address answers this Foundry's integrations page, and the stored key, the Activity and the version every replica follows are as they were.
```

```text
Where no key is held to seal a new one with, she is not offered a way out that could not work, and the file to restore is the one thing she is told about.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:While this Foundry cannot read its instance key file\, starting over is not offered

```text
A page that withholds the way out wherever the secrets key does not open; the first scenario is what refuses that.
```

###### Obligations



##### Covers

P7

```text
Nothing saved, the Slack app she entered alone, one workspace connection alone, and a connection with the app she entered, each with a stored secrets key this Foundry cannot open; the instance admin opens the way out.
```

```text
The cost sentence of that count and no other: no saved credentials to discard; one saved credential discarded for the app alone and for the connection alone; two for the connection with the app. Each reads the confirmation it asks her to type, nothing claims a key was generated, and the stored key, the Activity and the version every replica follows are as they were.
```

```text
What she is told starting over costs matches what this Foundry actually holds, in each of these states.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:The start over page says what starting over discards\, and reading it discards nothing

```text
A count that counts each secret of the Slack app separately, or counts the app Foundry's environment sets as a saved credential; the connection-alone row is what refuses the second.
```

###### Obligations



##### Covers

P6

```text
The replace address of the secrets key opened, then posted, while this Foundry cannot open the stored key; then the instance key it was sealed with is set again.
```

```text
Both answer this Foundry's integrations page, with the stored key, the Activity and the version every replica follows as they were, and with the instance key back the page reads the secrets key as set and her saved Slack connection still opens.
```

```text
A replacement that could never seal her credentials again is neither offered to her nor carried out behind her back by a link anyone kept.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:While this Foundry cannot open its secrets key\, replacing it changes nothing

```text
A door that answers the page while a saved credential no held key opens would still let a replacement through; that state is Deliverable A's own I-4.
```

###### Obligations



##### Covers

P6

P7

```text
A Foundry whose stored secrets key opens, with a workspace connection and the Slack app she entered saved: the instance admin reads the page, then the start over address is opened on it, as a kept link or a link preview would.
```

```text
The page reads the secrets key as set, offers the way to replace it and no way to start over. The start over address answers this Foundry's integrations page, with the stored key, the Activity and the version every replica follows as they were, and her saved Slack connection still opens.
```

```text
The page that discards every credential she cannot open is not a page a healthy Foundry ever shows her, and nobody reaches it with a kept link.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:While this Foundry opens its secrets key\, starting over is not offered and changes nothing

```text
A door guarded on the key mode and one guarded on a saved-credential count it cannot read are not told apart by this case; a post to that address while the key opens is observed where its route lives, with the apply.
```

###### Obligations




## Expectations

### Rows

#### E1

##### Promises

P1

```text
On GET /admin/instance/integrations, while the stored secrets key opens, the user sees a Replace secrets key action next to the secrets-key status and can open it.
```

#### E2

##### Promises

P1

```text
GET /admin/instance/integrations/secrets-key/replace shows a Replace secrets key button and one sentence that matches the number of saved credentials: for two or more, Replace the secrets key? Foundry seals its {n} saved credentials again under a new key. Nobody needs to reconnect.; for exactly one, the same sentence with 1 saved credential; for none, Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again.
```

#### E3

##### Promises

P1

```text
Opening the replace page, also more than once or as a browser prefetch, changes nothing the user can see: no new key, no re-sealing, no Activity line, and no message on the integrations page afterwards.
```

#### E4

##### Promises

P2

```text
When the user confirms Replace secrets key, Foundry returns the user to GET /admin/instance/integrations, which shows once: Replaced. {n} saved credentials are sealed under the new key. for two or more, Replaced. 1 saved credential is sealed under the new key. for exactly one, and Replaced. This Foundry has a new secrets key. for none.
```

#### E5

##### Promises

P2

```text
After a confirmed replacement, every saved credential still opens, and no person must reconnect a connection or enter a token again.
```

#### E6

##### Promises

P2

```text
When the user reloads the instance integrations page after a replacement, the replaced message does not come back, and the Activity shows one replacement only.
```

#### E7

##### Promises

P3

```text
When the re-sealing fails, the user sees no replaced message and no replacement in the Activity, and all saved credentials still open with the key that was in effect before.
```

#### E8

##### Promises

P4

```text
Within 60 seconds after a replacement, every running Foundry server opens every saved credential: GET /workspace/integrations and the route history show no credential that cannot be opened.
```

#### E9

##### Promises

P4

```text
A replacement interrupts no integration: no connection is shown as lost, no workspace admin receives an email, and the next new issue of a team member still reaches its Slack channel and its Matrix room.
```

#### E10

##### Promises

P5

```text
The Activity on GET /admin/instance/integrations shows, with the date and the name of the person, a line that the person replaced the secrets key with the number of credentials sealed again, and a line that the person started over with a new secrets key with the number of credentials discarded.
```

#### E11

##### Promises

P6

```text
While the stored secrets key cannot be opened and no running replica opens it, GET /admin/instance/integrations adds Start over to the notice that the key cannot be opened, and offers no Replace secrets key.
```

#### E12

##### Promises

P6

```text
While another live Foundry server still opens the secrets key, the notice stays restore-only: it offers no Start over and tells the user: Another Foundry server still opens the secrets key, so Start over is not offered.
```

#### E14

##### Promises

P6

```text
While the stored secrets key cannot be opened, the replace route changes nothing: a GET and a POST to /admin/instance/integrations/secrets-key/replace answer with the integrations page, and no replacement occurs.
```

#### E15

##### Promises

P7

```text
GET /admin/instance/integrations/secrets-key/start-over is answered only while the stored secrets key cannot be opened and no running replica opens it; in all other conditions the user does not get that page.
```

#### E16

##### Promises

P7

```text
The start-over page has a Confirmation field with this text: Type START OVER to confirm that no Foundry server of an earlier release is still running.
```

#### E17

##### Promises

P7

```text
The start-over page states the cost for the number of credentials this Foundry cannot open: for two or more, Start over with a new secrets key? The {n} saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again.; for exactly one, the same sentence with The 1 saved credential this Foundry cannot open is discarded; for none, Start over with a new secrets key? This Foundry has no saved credentials to discard.
```

#### E18

##### Promises

P7

```text
Opening the start-over page changes nothing the user can see.
```

#### E19

##### Promises

P8

```text
When the user types START OVER in the Confirmation field and confirms, Foundry makes a new secrets key, discards the credentials it cannot open, records the started-over line in the Activity, and returns the user to GET /admin/instance/integrations, which shows once: Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.
```

#### E20

##### Promises

P8

```text
A wrong or empty Confirmation, or a running replica that still opens the key, shows the start-over cost page again and changes nothing: no new key, no discarded credential and no Activity line.
```

#### E21

##### Promises

P9

```text
After a start-over, Matrix is ready, and the instance integrations page asks for the Slack app again.
```

#### E22

##### Promises

P9

```text
After a start-over, GET /workspace/integrations shows no discarded connection as connected, and tells the workspace admin to reconnect Slack and to re-enter the Matrix bot token.
```

#### E23

##### Promises

P10

```text
After a replacement or a start-over, the CLI command foundry doctor rotate-secrets completes, and every saved credential still opens afterwards.
```

#### E24

##### Promises

P11

```text
With 3 saved credentials and a secrets key that opens, Priya confirms Replace secrets key and reads Replaced. 3 saved credentials are sealed under the new key.; afterwards the next new issue of Sam in Identity Platform reaches #auth-dev and #auth:canzan.dev, no connection is marked lost, and no workspace admin is emailed.
```

#### E25

##### Promises

P12

```text
With a stored secrets key this server cannot open, 3 credentials it cannot open and no running replica that opens the key, Priya reads Start over with a new secrets key? The 3 saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again. and nothing has changed before she confirms; after she types START OVER and confirms, she reads Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it. once, and the workspace integrations page of Canzan Labs offers to connect Matrix and does not read Slack as connected.
```

#### E26

##### Promises

P6

```text
While this Foundry cannot read its instance key file at all, GET /admin/instance/integrations shows in the place of the instance key line the message that this Foundry cannot read its instance key file and that an instance operator restores the file from backup or sets FOUNDRY_INSTANCE_KEY and restarts Foundry; the user reads none of the words of the notice that the stored secrets key cannot be opened, and the page offers no Replace secrets key, no Start over and no sentence about another Foundry server.
```

## Charter

None.

## Examination

None.

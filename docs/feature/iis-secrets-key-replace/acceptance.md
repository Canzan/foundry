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




##### Covers

P6

```text
A second Foundry server on the same database still opens the stored secrets key while this one cannot, and it is read in the quiet between two of its own words: longer than the gap it speaks at, and less than the time such a word counts as current for. The instance admin opens the instance integrations page and then the start over address.
```

```text
The restore words, followed by the sentence that another Foundry server still opens the secrets key so starting over is not offered, with no way to start over and no way to replace the key - exactly what she reads while that server has just spoken. The start over address answers this Foundry's integrations page, and the stored key, the Activity and the version every replica follows are as they were.
```

```text
A running server that is simply between two of its words still counts as a server using the credentials, so she is never offered a way out that would discard what it is serving with.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Between two words of that server\, starting over is still not offered

```text
A gate that keeps the server live for longer than the whole window it should; this case only pins the quiet inside the window, and the stopped-server scenario is what refuses a window that never ends.
```

###### Obligations



##### Covers

P6

```text
The gate is asked whether a server whose own word says it opens the stored secrets key still counts as running, over the setting of the writing server, the setting of the reading server and the age of that word, generated together: settings mixed in both directions, equal settings on both sides, a reading setting larger than the writing one, a word dated ahead of the reading clock, and a word that says the server does not open the key at every age.
```

```text
Such a word counts exactly while it is at most two beat intervals plus the reload envelope old, whatever either setting is; a word that says the server does not open the key never counts at any age; and the answer is reached from the cadence and the envelope alone, with no setting handed to the gate.
```

```text
Whatever each of her Foundry servers is set to, she is withheld the way out while any of them is still using the credentials, and is offered it once none of them is.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:\:T3

```text
A gate that answers this range correctly while the page that offers the way out never asks it; the two page runs of this value are what refuse that.
```

###### Obligations










##### Covers

P6

```text
The time a word about opening the stored secrets key counts as current for is compared with the hour a server's record of itself is kept for, at both ends of the admitted range of the reload setting on either side, with the cadence and the retention read where each of them is owned.
```

```text
That time is the same length at every setting of every server and stays well inside the hour such a record is kept, so a word the gate would still count on can never have been forgotten first.
```

```text
She is never withheld the way out on the word of a server whose record of itself has already been swept away, at any setting of any of her servers.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:\:T4

```text
A time inside that hour that is nonetheless too short for the cadence a running server speaks at; the generated projection over the settings and the ages is what refuses that.
```

###### Obligations






##### Covers

P6

```text
The setting that says how often a Foundry server reloads its integrations is read the way a server reads it as it boots, over everything a deployment can leave in it, generated: nothing at all, something that is not a whole number of milliseconds, zero, one, every value up to twenty seconds, twenty seconds itself, the first value past it, a minute, and the largest number there is.
```

```text
Every value up to twenty seconds inclusive is admitted and read as given, with nothing in the setting read as the five-second default and zero read as one millisecond. Every value above twenty seconds is refused where the setting is read, with no settings value produced at all, so the server refuses to start instead of serving, and the refusal names the setting, the value it read and the bound. Something that is not a whole number is still refused as that and not as one out of range. The admitted and refused sets are worked out from the bound itself, not from what the read answers.
```

```text
No operator can leave her Foundry serving with a reload so slow that its own running servers look gone to one another and she is offered a way out that would discard the credentials they are still serving with: such a setting stops the server as it starts and says what to set instead.
```

##### Disposition

Tested

crates\/foundry\-app\/tests\/iis\_secrets\_key\_bound\.rs\:\:the\_reload\_setting\_is\_admitted\_only\_up\_to\_the\_envelope

```text
A refusal whose words differ from the admitted range the operator documentation states: this check reads the setting, the value and the bound in the refusal but pins none of the surrounding prose, so review and the examiner are what refuse that.
```

###### Obligations










### S3

##### Covers

P8

P9

P5

P12

```text
This Foundry cannot open its stored secrets key, the Slack app Priya entered and Canzan Labs' Slack and Matrix connections cannot be opened, and no running Foundry server opens the key. Priya opens the start over page, reads what it costs, types START OVER and confirms; afterwards she asks for the address she landed on again.
```

```text
The start over page states the cost of discarding the 3 saved credentials this Foundry cannot open, and reading it changes neither the stored key nor the whole Activity list. Confirming lands her on the instance integrations page, which reads once that the secrets key is generated, Matrix is ready and the Slack app can now be entered; the address she lands on carries nothing, a stored secrets key this Foundry opens is in place with its key check naming it, no connection is marked lost and no workspace admin is emailed. Asking for that same address again reads the stored key as set, asks for the Slack app again, says nothing about a key being generated, and the Activity list reads exactly the started-over line with its count of 3 above the two shipped lines, in the words a reader of another release reads. Canzan Labs' workspace integrations page reads no connection as connected, offers to connect Matrix and reads the shipped words that Slack is not set up on this Foundry; no page carries the new key in any encoding.
```

```text
Priya gets out of a lost instance key with no backup at the cost she read first: a key that works again, Matrix ready, the Slack app asked for again, and Canzan Labs told to set its connections up again rather than left reading a connection that cannot post.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Starting over makes a secrets key this Foundry opens and leaves the workspace told to set up again

```text
A start over that discards every credential this Foundry cannot open but leaves one it can open behind: under a stored key this server cannot open there is no credential it opens, so that half is observed where a replacement is, and the kept connection rows and their routes are read through the workspace page alone.
```

###### Obligations


##### Covers

P8

```text
This Foundry cannot open its stored secrets key, two saved credentials cannot be opened, and no running Foundry server opens the key. Priya pastes the confirmation with a space before and after it and confirms the start over.
```

```text
The start over is applied: she lands on the instance integrations page reading once that the secrets key is generated, Matrix is ready and the Slack app can now be entered, a stored secrets key this Foundry opens is in place, and the Activity reads the started-over line with its count of 2 exactly once.
```

```text
Priya is not refused for the spaces a paste brings along: the words she was asked to type are the words, whatever surrounds them.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:The Confirmation is read with the spaces a paste brings along trimmed

```text
A confirmation read with any whitespace anywhere removed rather than only around the words: a value with a space inside the words is not tried here.
```

###### Obligations

None.

##### Covers

P8

```text
This Foundry cannot open its stored secrets key, two saved credentials cannot be opened, and no running Foundry server opens the key. Priya confirms the start over with the field left empty, and again with the words in lower case; afterwards an operator restores the instance key the stored key was sealed with and Foundry restarts.
```

```text
Each attempt is answered with the start over page stating its cost for two credentials again, says nothing about a key being generated, and leaves the stored key, the whole Activity list and the version every server follows as they were. With the instance key restored the page reads the stored secrets key as set, Canzan Labs' saved Slack connection opens again, and the Slack app Priya entered is still in effect with both its secrets and its activity unchanged.
```

```text
Nothing is discarded unless Priya types the words exactly: a misread click, a stray submit or an empty field costs her nothing, and everything she still has keeps working once the instance key is back.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:A start over confirmed with other words than START OVER discards nothing

```text
A Foundry that refuses every start over whatever is typed: only the accepted rows tell that apart, which is why they are recorded beside this one.
```

###### Obligations

None.

##### Covers

P8

```text
A second Foundry server on this database still opens the stored secrets key while this one cannot. That server says nothing for a minute, so Priya reads the start over page and its cost; it then says again that it opens the stored key, and she types START OVER and confirms the form she is on. Afterwards an operator restores the instance key the stored key was sealed with and Foundry restarts.
```

```text
The confirmation is answered with the start over page and its cost again, says nothing about a key being generated, and leaves the stored key, the whole Activity list and the version every server follows as they were; with the instance key restored the page reads the stored secrets key as set and Canzan Labs' saved Slack connection opens again.
```

```text
No credential a running Foundry server is still serving with is discarded, even when that server goes quiet while Priya reads the page and speaks again before she confirms.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:A start over confirmed after another Foundry server speaks again discards nothing

```text
A Foundry that reads the state of the other servers once more anywhere in the confirming request rather than inside the write that discards: a server that speaks between that read and the write is not observed here.
```

###### Obligations



##### Covers

P10

```text
After a start over Priya enters the Slack app again, and the operator then rotates the instance key itself with the key-rotation command of the Foundry binary.
```

```text
The command reports no credential sealed again under the new key and succeeds, the Slack app Priya re-entered is still in effect with both its secrets and its activity unchanged, Slack's events still verify with the secret it was saved with, and the instance integrations page reads the stored secrets key as set.
```

```text
The operator's own key rotation stays safe after a start over: what Priya entered afterwards keeps opening, with nothing to re-enter.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:After a start over the operator\'s own key rotation still leaves every credential opening

```text
A rotation run while nothing had been entered again after the start over: the Slack app re-entered here is the one credential it is read over, and a workspace connection made again afterwards is not.
```

###### Obligations


##### Covers

P8

```text
This Foundry opens its stored secrets key and holds one workspace connection and the Slack app Priya entered. A kept link asks for the start over address, and a stale tab posts to it.
```

```text
Both are answered with this Foundry's own integrations page, neither reads anything about a key being generated, and the stored key, the whole Activity list and the version every server follows are as they were; Canzan Labs' saved Slack connection still opens and the Slack app is still in effect with both its secrets and its activity unchanged.
```

```text
A healthy Foundry never discards anything through the start-over address: an old link or a tab left open posts nothing away.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:While this Foundry opens its secrets key\, starting over is not offered and changes nothing

```text
A Foundry that refuses this post only because the kept form carries no confirmation: the typed confirmation is posted to this address only where starting over is offered.
```

###### Obligations

None.

##### Covers

P8

```text
A replacement whose re-seal refuses, at the first workspace connection credential and again at the saved Slack app's secrets, and a replacement whose stored key is no longer the one it was planned from.
```

```text
Each refusal stops at the slot it refused on and leaves every stored row as it was: the old key still in effect, every saved credential still sealed under it, no activity line and no version bump; the lost compare-and-set leaves the first replacement standing as the only recorded one.
```

```text
A replacement that cannot finish costs Priya nothing: every credential she had keeps opening and nothing claims a rotation that did not happen.
```

##### Disposition

Tested

crates\/foundry\-store\/tests\/iis\_instance\_store\.rs\:\:a\_refused\_replacement\_and\_a\_lost\_compare\_and\_set\_leave\_everything\_unchanged

```text
A refusal observed at the write and not through the page: what Priya reads after a refused replacement is observed where the door is, and this row reads the stored rows only.
```

###### Obligations




##### Covers

P8

```text
Two replace requests reach two Foundry servers of one database at the same moment while a workspace connection is routed to a Slack channel, and a new issue is filed afterwards.
```

```text
The key is replaced once, both requests are answered with the resulting page, within 60 seconds and with no restart every server opens every credential, no connection is marked lost, no workspace admin is emailed, and every server's route history lists the new issue as delivered.
```

```text
The window in which the servers hold different keys costs the workspace nothing: deliveries still go out, nobody is told a connection was lost and nobody reconnects.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Two replace requests at the same moment replace the key once\, and every replica opens every credential

```text
A delivery attempted only after every server had already reloaded: the attempt is made while the window is open but its exact moment inside the window is not controlled here.
```

###### Obligations




##### Covers

P6

```text
This Foundry loses the instance key its stored secrets key was sealed with while a second Foundry server of the same database still opens that key; that server then stops, and a minute passes.
```

```text
While the other server still opens the key the page stays restore-only, adds that another Foundry server still opens the secrets key and offers neither way on; once its last word is a minute old the page offers Start over and the start over page states its cost for the credentials this Foundry cannot open.
```

```text
Priya is never offered a way out that would discard what another running server is still serving with, and she is offered it as soon as that server is gone.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:Once that server has stopped\, starting over is offered with its cost

```text
A page that reads the other server's word rather than how old it is: the two states are told apart here only by that server stopping, which is what makes its word age.
```

###### Obligations


##### Covers

P8

```text
The architecture check reads every crate's sources for a write of a sealed column or of the stored secrets key, and for a module outside this Foundry's integrations that opens or seals a credential.
```

```text
Every such write stands inside a listed fenced writer of the store's instance module, the two new writes included, and nothing outside the integrations module opens or seals a credential; the check names any other writer and fails.
```

```text
No part of Foundry can quietly write a sealed credential or the stored key outside the one place that keeps a replacement and a start over all-or-nothing.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs\:\:integrations\-seal\-fence

```text
A writer that is not Rust source of this repository: a direct database write or a restored backup is outside what this check can read.
```

###### Obligations


##### Covers

P8

```text
Priya starts over under a stored key this Foundry cannot open, then enters the Slack app again and reconnects Canzan Labs to Slack. On the key the start over made, she replaces the secrets key.
```

```text
After the start over the page offers a way to replace the secrets key and no way to start over; the replacement reads that the 2 saved credentials are sealed under the new key, a different secrets key is stored, the page again offers only a replacement, Canzan Labs' saved Slack connection still opens and the Slack app is still in effect with both its secrets and its activity unchanged.
```

```text
Starting over leaves Priya a Foundry she can go on rotating: what she saved after it is sealed again under the next key, and nobody reconnects for that.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:A replacement after a start over seals what Priya saved since under the new key

```text
A Foundry whose two ways on are decided per request rather than from the stored key's one state: each state is read here after an apply, and the decision itself is read only through what the page offers.
```

###### Obligations



##### Covers

P8

```text
Priya replaces the secrets key, and this Foundry then restarts with an instance key that cannot open the key she just made. She starts over from the page it leaves her.
```

```text
The page offers a way to start over and no Replace; the start over reads once that the secrets key is generated, Matrix is ready and the Slack app can now be entered, a stored secrets key this Foundry opens is in place, the page then offers a way to replace it and no way to start over, and Canzan Labs' workspace integrations page offers to connect Matrix.
```

```text
A replacement is no protection against the instance key going missing, and the way out is the same one either way: Priya ends on a Foundry whose key she can replace again, with Matrix ready.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:A start over after a replacement leads back to a Foundry whose key can be replaced

```text
A start over that leaves the credentials of the replaced key readable: everything saved before the restart is a credential this Foundry cannot open, so what survives a start over is read where a credential saved after it is.
```

###### Obligations



##### Covers

P8

```text
While this Foundry cannot open its stored secrets key, the instance admin opens the start-over page and reads its cost. An instance operator then restores the instance key the stored key was sealed with and restarts Foundry, so the stored key and the 2 saved credentials open again. From the tab she left open she confirms the start over with the Confirmation typed as START OVER.
```

```text
The confirm is answered with this Foundry's own integrations page. Nothing she reads there asks the start-over cost question, and no way to start over is offered. The stored secrets key, the Activity and the version every replica follows are as they were, her saved Slack connection still opens, and the saved Slack app keeps its secrets and its activity.
```

```text
A confirm kept in an open tab cannot discard the credentials this Foundry can open again: the admin lands back on her integrations page with every connection and the saved Slack app intact, and is offered no way to discard them.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:1375 While this Foundry opens its secrets key\, a start over confirmed from a tab left open changes nothing

```text
A Foundry that answers its own integrations page here because it refuses every start-over confirm whose form it did not draw in this request (a one-shot form token), rather than because the stored secrets key opens on this server.
```

###### Obligations


##### Covers

P8

```text
This Foundry cannot open its stored secrets key and two saved credentials cannot be opened. The instance admin reads the way out while the second Foundry server of this database has been quiet long enough to be gone; that server speaks again, and she confirms with the typed confirmation in the quiet between that word and its next - longer than the gap it speaks at, and less than the time such a word counts as current for.
```

```text
Her confirm is answered the way out again, stating what starting over discards, with nothing claiming a secrets key was generated; the stored key, the Activity and the version every replica follows are as they were, and with the instance key set again the page reads the secrets key as set and her saved Slack connection still opens.
```

```text
The credentials a running server is still serving with are never discarded under it, not even when she confirms in the quiet between two of its words.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature\:\:A start over confirmed between two words of another Foundry server discards nothing

```text
A confirm refused for the wrong reason - a gate read only at the page she read, with her confirm refused by its own staleness; the scenario where that server speaks again after she read the page is what makes the confirm-time read the one that answers.
```

###### Obligations


##### Covers

P8

```text
The gate the confirm re-reads inside its own locked write is asked whether a server whose own word says it opens the stored secrets key still counts as running, over the setting of the writing server, the setting of the reading server and the age of that word, generated together: settings mixed in both directions, equal settings on both sides, a reading setting larger than the writing one, a word dated ahead of the reading clock, and a word that says the server does not open the key at every age.
```

```text
Such a word counts exactly while it is at most two beat intervals plus the reload envelope old, whatever either setting is; a word that says the server does not open the key never counts at any age; and the answer is reached from the cadence and the envelope alone, with no setting handed to the gate, for the confirm exactly as for the page.
```

```text
Whatever each of her Foundry servers is set to, her confirm never discards credentials a server is still using, and goes through once none of them is using them.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:\:T5

```text
A gate that answers this range correctly while the confirm asks it only at the page she read; the run where that server speaks again after she read the page is what refuses that.
```

###### Obligations










##### Covers

P8

```text
The time a word about opening the stored secrets key counts as current for, the same time the confirm re-reads it against, is compared with the hour a server's record of itself is kept for, at both ends of the admitted range of the reload setting on either side, with the cadence and the retention read where each of them is owned.
```

```text
That time is the same length at every setting of every server and stays well inside the hour such a record is kept, for the confirm as for the page, so a word the confirm would still refuse on can never have been forgotten first.
```

```text
Her confirm is never refused on the word of a server whose record of itself has already been swept away, at any setting of any of her servers.
```

##### Disposition

Tested

crates\/foundry\-app\/src\/integrations\/secrets\_key\_rotation\.rs\:\:T6

```text
A time inside that hour that is nonetheless too short for the cadence a running server speaks at; the generated projection over the settings and the ages is what refuses that.
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

None.

None.

None.

None.

#### E2

##### Promises

P1

```text
GET /admin/instance/integrations/secrets-key/replace shows a Replace secrets key button and one sentence that matches the number of saved credentials: for two or more, Replace the secrets key? Foundry seals its {n} saved credentials again under a new key. Nobody needs to reconnect.; for exactly one, the same sentence with 1 saved credential; for none, Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again.
```

None.

None.

None.

None.

#### E3

##### Promises

P1

```text
Opening the replace page, also more than once or as a browser prefetch, changes nothing the user can see: no new key, no re-sealing, no Activity line, and no message on the integrations page afterwards.
```

None.

None.

None.

None.

#### E4

##### Promises

P2

```text
When the user confirms Replace secrets key, Foundry returns the user to GET /admin/instance/integrations, which shows once: Replaced. {n} saved credentials are sealed under the new key. for two or more, Replaced. 1 saved credential is sealed under the new key. for exactly one, and Replaced. This Foundry has a new secrets key. for none.
```

None.

None.

None.

None.

#### E5

##### Promises

P2

```text
After a confirmed replacement, every saved credential still opens, and no person must reconnect a connection or enter a token again.
```

None.

None.

None.

None.

#### E6

##### Promises

P2

```text
When the user reloads the instance integrations page after a replacement, the replaced message does not come back, and the Activity shows one replacement only.
```

None.

None.

None.

None.

#### E7

##### Promises

P3

```text
When the re-sealing fails, the user sees no replaced message and no replacement in the Activity, and all saved credentials still open with the key that was in effect before.
```

None.

None.

None.

None.

#### E8

##### Promises

P4

```text
Within 60 seconds after a replacement, every running Foundry server opens every saved credential: GET /workspace/integrations and the route history show no credential that cannot be opened.
```

None.

None.

None.

None.

#### E9

##### Promises

P4

```text
A replacement interrupts no integration: no connection is shown as lost, no workspace admin receives an email, and the next new issue of a team member still reaches its Slack channel and its Matrix room.
```

None.

None.

None.

None.

#### E10

##### Promises

P5

```text
The Activity on GET /admin/instance/integrations shows, with the date and the name of the person, a line that the person replaced the secrets key with the number of credentials sealed again, and a line that the person started over with a new secrets key with the number of credentials discarded.
```

None.

None.

None.

None.

#### E11

##### Promises

P6

```text
While the stored secrets key cannot be opened and no running replica opens it, GET /admin/instance/integrations adds Start over to the notice that the key cannot be opened, and offers no Replace secrets key.
```

None.

None.

None.

None.

#### E12

##### Promises

P6

```text
While another live Foundry server still opens the secrets key, the notice stays restore-only: it offers no Start over and tells the user: Another Foundry server still opens the secrets key, so Start over is not offered.
```

None.

None.

None.

None.

#### E14

##### Promises

P6

```text
While the stored secrets key cannot be opened, the replace route changes nothing: a GET and a POST to /admin/instance/integrations/secrets-key/replace answer with the integrations page, and no replacement occurs.
```

None.

None.

None.

None.

#### E15

##### Promises

P7

```text
GET /admin/instance/integrations/secrets-key/start-over is answered only while the stored secrets key cannot be opened and no running replica opens it; in all other conditions the user does not get that page.
```

None.

None.

None.

None.

#### E16

##### Promises

P7

```text
The start-over page has a Confirmation field with this text: Type START OVER to confirm that no Foundry server of an earlier release is still running.
```

None.

None.

None.

None.

#### E17

##### Promises

P7

```text
The start-over page states the cost for the number of credentials this Foundry cannot open: for two or more, Start over with a new secrets key? The {n} saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again.; for exactly one, the same sentence with The 1 saved credential this Foundry cannot open is discarded; for none, Start over with a new secrets key? This Foundry has no saved credentials to discard.
```

None.

None.

None.

None.

#### E18

##### Promises

P7

```text
Opening the start-over page changes nothing the user can see.
```

None.

None.

None.

None.

#### E19

##### Promises

P8

```text
When the user types START OVER in the Confirmation field and confirms, Foundry makes a new secrets key, discards the credentials it cannot open, records the started-over line in the Activity, and returns the user to GET /admin/instance/integrations, which shows once: Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it.
```

None.

None.

None.

None.

#### E20

##### Promises

P8

```text
A wrong or empty Confirmation, or a running replica that still opens the key, shows the start-over cost page again and changes nothing: no new key, no discarded credential and no Activity line.
```

None.

None.

None.

None.

#### E21

##### Promises

P9

```text
After a start-over, Matrix is ready, and the instance integrations page asks for the Slack app again.
```

None.

None.

None.

None.

#### E22

##### Promises

P9

```text
After a start-over, GET /workspace/integrations shows no discarded connection as connected, and tells the workspace admin to reconnect Slack and to re-enter the Matrix bot token.
```

None.

None.

None.

None.

#### E23

##### Promises

P10

```text
After a replacement or a start-over, the CLI command foundry doctor rotate-secrets completes, and every saved credential still opens afterwards.
```

None.

None.

None.

None.

#### E24

##### Promises

P11

```text
With 3 saved credentials and a secrets key that opens, Priya confirms Replace secrets key and reads Replaced. 3 saved credentials are sealed under the new key.; afterwards the next new issue of Sam in Identity Platform reaches #auth-dev and #auth:canzan.dev, no connection is marked lost, and no workspace admin is emailed.
```

None.

None.

None.

None.

#### E25

##### Promises

P12

```text
With a stored secrets key this server cannot open, 3 credentials it cannot open and no running replica that opens the key, Priya reads Start over with a new secrets key? The 3 saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again. and nothing has changed before she confirms; after she types START OVER and confirms, she reads Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it. once, and the workspace integrations page of Canzan Labs offers to connect Matrix and does not read Slack as connected.
```

None.

None.

None.

None.

#### E26

##### Promises

P6

```text
While this Foundry cannot read its instance key file at all, GET /admin/instance/integrations shows in the place of the instance key line the message that this Foundry cannot read its instance key file and that an instance operator restores the file from backup or sets FOUNDRY_INSTANCE_KEY and restarts Foundry; the user reads none of the words of the notice that the stored secrets key cannot be opened, and the page offers no Replace secrets key, no Start over and no sentence about another Foundry server.
```

None.

None.

None.

None.

## Charter

None.

## Examination

### E1

met

#### Observations

```text
State three-credentials-key-opens, GET admin/instance/integrations as Priya: the page reads Secrets key: set. Credentials entered here are sealed with it. and, in that same secrets key line, offers the action Replace secrets key linking to /admin/instance/integrations/secrets-key/replace, which the probe then opened.
```

### E2

met

#### Observations

```text
GET admin/instance/integrations/secrets-key/replace, three counts. With 3 saved credentials: Replace the secrets key? Foundry seals its 3 saved credentials again under a new key. Nobody needs to reconnect. With 1 saved credential (state slack-app-saved): Replace the secrets key? Foundry seals its 1 saved credential again under a new key. Nobody needs to reconnect. With none (state secrets-key-no-slack): Replace the secrets key? This Foundry has no saved credentials yet, so nothing needs to be sealed again. Each page carried a Replace secrets key button posting to the same address.
```

### E3

met

#### Observations

```text
The replace confirm page was fetched twice in a row (the second as a prefetch) and answered the same sentence both times. The instance integrations page fetched right after the two GETs was word for word the one fetched before them: no notice, Secrets key: set unchanged, and the Activity still only the two setup lines (saved the Slack app, generated the secrets key). No replacement line appeared.
```

### E4

met

#### Observations

```text
POST admin/instance/integrations/secrets-key/replace answered with final-url https://foundry.example.org/admin/instance/integrations, and that page read Replaced. 3 saved credentials are sealed under the new key. In the one-credential state the same POST read Replaced. 1 saved credential is sealed under the new key., and in the no-credential state Replaced. This Foundry has a new secrets key.
```

### E5

met

#### Observations

```text
After the confirmed replacement in three-credentials-key-opens with two replicas on one database, nobody reconnected anything and every credential still opened. Asked directly, replica 2 read the Canzan Labs workspace integrations page as Connected to Canzan Labs (canzanlabs.slack.com) and the Ops Guild page as Connected to Ops Guild (opsguild.slack.com), with no cannot-open or lost wording; the project route page on replica 2 read #auth-dev - Delivering - last delivery 18:05. A new issue filed on replica 2 after the replacement, AUTH-2, was posted to Slack: the Slack double recorded chat.postMessage team=Canzan Labs channel=C0AUTHDEV text=AUTH-2 After the replacement New issue in Identity Platform by Lena Park - Backlog. All of this happened inside the same probe run, well under a minute per command.
```

### E6

met

#### Observations

```text
The instance integrations page reloaded twice after the replacement read no notice at all, and its Activity held exactly one replacement line, 15 Nov 2026 - Priya Raman replaced the secrets key (3 credentials sealed again). Nothing was posted again and no second replacement was claimed.
```

### E7

met

#### Observations

```text
State three-credentials-one-damaged (one stored sealed credential damaged so it no longer opens): the confirm page still read Replace the secrets key? Foundry seals its 3 saved credentials again under a new key., and POST of it answered that same confirm page rather than a redirect. The instance integrations page afterwards read no Replaced notice and its Activity held only the two setup lines, with no replacement line; the workspace integrations page still read Connected to Canzan Labs. The old key stayed in effect and the pages were unchanged.
```

### E8

met

#### Observations

```text
After the confirmed replacement in three-credentials-key-opens with two replicas on one database, nobody reconnected anything and every credential still opened. Asked directly, replica 2 read the Canzan Labs workspace integrations page as Connected to Canzan Labs (canzanlabs.slack.com) and the Ops Guild page as Connected to Ops Guild (opsguild.slack.com), with no cannot-open or lost wording; the project route page on replica 2 read #auth-dev - Delivering - last delivery 18:05. A new issue filed on replica 2 after the replacement, AUTH-2, was posted to Slack: the Slack double recorded chat.postMessage team=Canzan Labs channel=C0AUTHDEV text=AUTH-2 After the replacement New issue in Identity Platform by Lena Park - Backlog. All of this happened inside the same probe run, well under a minute per command.
```

### E9

not_examined

```text
Half of this row was observed holding and half cannot be captured. Observed: after the replacement no page read any connection as lost, the route stayed Delivering, the mail double held only the two setup invitations with no message to any workspace admin, and the next new issue reached its Slack channel #auth-dev (chat.postMessage channel=C0AUTHDEV). Not examined: that the next new issue still reaches its Matrix room. Every Matrix-bearing probe state states that Foundry reaches no Matrix homeserver this probe can run, and the public projection of this packet is empty, so no waiver records that limit. Prerequisite: a public observation of a new issue reaching a Matrix room, or a waiver carried in the public packet. Owner: the evidence-capture owner of the examine-foundry harness, with the acceptance designer. Remedy: add a Matrix homeserver double to the probe, or publish the Matrix waiver in the examination packet. No violation was observed.
```

### E10

met

#### Observations

```text
The Activity on GET admin/instance/integrations read, with date and person, 15 Nov 2026 - Priya Raman replaced the secrets key (3 credentials sealed again) after the replacement, and 15 Nov 2026 - Priya Raman started over with a new secrets key (3 credentials discarded) after the start over. Neither line read secrets key replaced or secrets key started over, and both counts matched the credentials involved.
```

### E11

met

#### Observations

```text
State secrets-key-unopenable (3 credentials, no running server opens the key): GET admin/instance/integrations read This Foundry cannot open its secrets key: the instance key it was sealed with is missing or different on this server. Until it is restored, saved connections cannot be used. An instance operator restores that instance key, or sets it as the previous instance key, and restarts Foundry. followed by Start over, linking to /admin/instance/integrations/secrets-key/start-over. The page offered no Replace secrets key action and no replace link. The same page in the one-credential and no-credential unopenable states also offered Start over and no Replace.
```

### E12

met

#### Observations

```text
State secrets-key-unopenable-live-server (replica 2 still runs with the instance key that sealed the stored key): GET admin/instance/integrations on the server that cannot open the key read the same restore words and then Another Foundry server still opens the secrets key, so Start over is not offered. The page listed no link at all: no Start over, no Replace secrets key. The restore words themselves were unaltered; the live-server sentence was added after them.
```

```text
Limit of the live-opener observations: the probe starts and stops replicas but gives no way to read the page at a chosen moment between two heartbeats of a replica that is alive and still opens the key. The live-opener runs were therefore read while replica 2 was running and beating normally. Whether a live opener read exactly between two beats is still treated as live was not observed either way.
```

### E14

met

#### Observations

```text
While the stored secrets key could not be opened, the replace route changed nothing. Under secrets-key-unopenable both GET and POST of admin/instance/integrations/secrets-key/replace answered the instance integrations page with the cannot-open notice; the page read afterwards was identical, with no Replaced notice and no new Activity line. Under the live-opener state the same GET and POST answered the restore-only page. Under instance-key-file-unreadable both answered the instance-key-file page. No replacement happened in any of these.
```

### E15

met

#### Observations

```text
GET admin/instance/integrations/secrets-key/start-over was answered as its own page only while the stored secrets key could not be opened and no running replica opened it: it answered the start-over page in secrets-key-unopenable, secrets-key-unopenable-one and secrets-key-unopenable-none. While the key opened (three-credentials-key-opens), while a live replica still opened it, and while the instance key file could not be read, the same GET answered the instance integrations page instead, never the start-over page.
```

### E16

met

#### Observations

```text
The start-over page carried the field text Type START OVER to confirm that no Foundry server of an earlier release is still running. with a field labelled Confirmation, in a form posting to /admin/instance/integrations/secrets-key/start-over with the single text field confirmation and the button Start over.
```

### E17

met

#### Observations

```text
The start-over page stated its cost for the count this Foundry cannot open. With 3: Start over with a new secrets key? The 3 saved credentials this Foundry cannot open are discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again. With 1 (secrets-key-unopenable-one): Start over with a new secrets key? The 1 saved credential this Foundry cannot open is discarded: workspace admins reconnect Slack and re-enter Matrix bot tokens, and the Slack app is entered again. With none (secrets-key-unopenable-none): Start over with a new secrets key? This Foundry has no saved credentials to discard.
```

### E18

met

#### Observations

```text
Opening the start-over page changed nothing a user can see: the instance integrations page read immediately after that GET was word for word the one read before it, still with the cannot-open notice and Start over, the Slack line still reading saved, but this Foundry cannot read it, and the Activity still only the two setup lines. The workspace integrations page was likewise unchanged.
```

### E19

met

#### Observations

```text
POST admin/instance/integrations/secrets-key/start-over with confirmation=START OVER answered with final-url https://foundry.example.org/admin/instance/integrations, and that page read Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it. once. The same page read twice more afterwards carried no notice. It read Secrets key: set., Matrix: ready., asked for the Slack app again with the manifest and a Save Slack app form, and its Activity read 15 Nov 2026 - Priya Raman started over with a new secrets key (3 credentials discarded).
```

### E20

met

#### Observations

```text
Three refusals of start over, each answering the cost page again with the I-28 sentence and the Confirmation field and changing nothing. An empty Confirmation; a Confirmation typed as start over in lower case; and confirmation=START OVER while a live replica still opened the key. After each one the instance integrations page was unchanged: still the cannot-open notice, no new key, the saved credentials still listed as unreadable rather than discarded, and no new Activity line. In the live-opener run the gate was applied inside the POST, since no start-over page had been answered for it at all.
```

```text
Limit of the live-opener observations: the probe starts and stops replicas but gives no way to read the page at a chosen moment between two heartbeats of a replica that is alive and still opens the key. The live-opener runs were therefore read while replica 2 was running and beating normally. Whether a live opener read exactly between two beats is still treated as live was not observed either way.
```

### E21

met

#### Observations

```text
POST admin/instance/integrations/secrets-key/start-over with confirmation=START OVER answered with final-url https://foundry.example.org/admin/instance/integrations, and that page read Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it. once. The same page read twice more afterwards carried no notice. It read Secrets key: set., Matrix: ready., asked for the Slack app again with the manifest and a Save Slack app form, and its Activity read 15 Nov 2026 - Priya Raman started over with a new secrets key (3 credentials discarded).
```

### E22

met

#### Observations

```text
After the start over, the Canzan Labs workspace integrations page read no discarded connection as connected: the Slack section read Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once. with the links How to set up Slack and Set up integrations for this Foundry, in place of the former Connected to Canzan Labs line, and the Matrix section offered a Connect Matrix form with Homeserver and Bot access token fields. The instance integrations page read Matrix: ready. and asked for the Slack app again.
```

### E23

met

#### Observations

```text
foundry doctor rotate-secrets ran with the settings replica 1 runs with and exited 0 both after a replacement and after a start over. After the replacement it reported 0 credential(s) sealed again under key fc3b0d508d0700d6, and a new issue filed afterwards still reached Slack: the double recorded chat.postMessage team=Canzan Labs channel=C0AUTHDEV text=AUTH-3 After the rotation New issue in Identity Platform by Lena Park - Backlog. After the start over it exited 0 and the instance page still read Secrets key: set.
```

### E24

not_examined

```text
The Slack half of this charter scenario was observed holding: with 3 saved credentials Priya read Replaced. 3 saved credentials are sealed under the new key., the next new issue of a team member (AUTH-2) reached #auth-dev, no page read a connection as lost, and no workspace admin was emailed. Not examined: that the same issue also reaches the Matrix room #auth:canzan.dev. The probe reaches no Matrix homeserver in any state and the public projection of this packet is empty, so no waiver records that limit. Prerequisite, owner and remedy as for E9: a Matrix homeserver double in examine-foundry or a published Matrix waiver, owned by the evidence-capture owner of the harness with the acceptance designer. No violation was observed.
```

### E25

met

#### Observations

```text
Opening the start-over page changed nothing a user can see: the instance integrations page read immediately after that GET was word for word the one read before it, still with the cannot-open notice and Start over, the Slack line still reading saved, but this Foundry cannot read it, and the Activity still only the two setup lines. The workspace integrations page was likewise unchanged.
```

```text
POST admin/instance/integrations/secrets-key/start-over with confirmation=START OVER answered with final-url https://foundry.example.org/admin/instance/integrations, and that page read Secrets key generated. Matrix is ready, and you can now enter the Slack app. Foundry keeps the key and never shows it. once. The same page read twice more afterwards carried no notice. It read Secrets key: set., Matrix: ready., asked for the Slack app again with the manifest and a Save Slack app form, and its Activity read 15 Nov 2026 - Priya Raman started over with a new secrets key (3 credentials discarded).
```

```text
After the start over, the Canzan Labs workspace integrations page read no discarded connection as connected: the Slack section read Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once. with the links How to set up Slack and Set up integrations for this Foundry, in place of the former Connected to Canzan Labs line, and the Matrix section offered a Connect Matrix form with Homeserver and Bot access token fields. The instance integrations page read Matrix: ready. and asked for the Slack app again.
```

### E26

met

#### Observations

```text
State instance-key-file-unreadable: GET admin/instance/integrations read, in the place of the instance key line, This Foundry cannot read its instance key file (/tmp/examine-foundry-data/instance.key). Slack and Matrix are paused until it can. An instance operator restores the file from backup, or sets FOUNDRY_INSTANCE_KEY, and restarts Foundry. No words of the cannot-open-the-secrets-key notice appeared, and the page offered no Replace secrets key, no Start over and no sentence about another Foundry server. The replace and start-over routes both answered this same page.
```

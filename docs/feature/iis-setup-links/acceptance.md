# Acceptance brief: iis\-setup\-links

## Slices

### S1

##### Covers

P1

```text
With this Foundry holding a secrets key and no Slack app, an instance admin opens the workspace integrations page and follows the offer she is given there.
```

```text
She reads the shipped sentence that Slack is not set up on this Foundry, then two offers in reading order, how to set up Slack and then setting up integrations for this Foundry, and following the second one shows her the instance integrations page.
```

```text
An instance admin fixes a missing Slack setup from the page where she noticed it, instead of being sent to a guide that tells an operator to edit settings.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin is sent from the workspace integrations page to this Foundry\'s setup

```text
the same setup offer were made to every viewer of the page rather than to an instance admin alone; the workspace-admin and lead rows are what refuse that behaviour.
```

###### Obligations




##### Covers

P1

P5

```text
With no secrets key and the Slack app given to this Foundry by its operator, an instance admin opens the workspace integrations page.
```

```text
She reads the shipped sentence that Slack cannot be connected until the secrets key is set and the shipped sentence that Matrix is not set up, and each of them is followed by the offer to set integrations up for this Foundry, both of which open for her.
```

```text
An instance admin sees, beside every integration the page says is missing, the one place she can fix it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin is offered the setup beside every integration the page says is missing

```text
the offer appeared beside only one of the two missing integrations; the Slack-door and Matrix-door rows pin each sentence separately.
```

###### Obligations


##### Covers

P3

P5

```text
With this Foundry holding a secrets key and no Slack app, a workspace admin who is not this Foundry's instance admin opens the workspace integrations page.
```

```text
She reads the same shipped sentence that Slack is not set up, is offered how to set up Slack and nothing else anywhere on the page, and that single offer opens for her.
```

```text
A workspace admin is never shown a setup she cannot open, so nothing on the page dead-ends her.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A workspace admin who is not an instance admin keeps the guide and is offered nothing she cannot open

```text
the setup offer were withheld from an instance admin as well; the instance-admin rows are what refuse that.
```

###### Obligations



##### Covers

P2

P5

```text
With this Foundry holding a secrets key and no Slack app, an instance admin opens a new Slack route for a project, asks that form for a preview, and saves it.
```

```text
Each of the three answers reads the same shipped sentence that Slack is not set up, with the same two offers in the same order, how to set up Slack and then setting up integrations for this Foundry, as the workspace integrations page shows for that state; each offer opens for her.
```

```text
An instance admin who meets a missing Slack setup while making a route is sent to the same fix, whichever of the form's doors she used.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin is sent to the setup from a Slack route form\, its preview and its save

```text
the form agreed with the page about the words while disagreeing about the order of the two offers; the order is stated as part of the expected list in every row. The save refuses in place rather than answering plainly, and the reader takes that re-rendered form as the page it is, so the row is decided by its offers and not by how the refusal is answered.
```

###### Obligations



##### Covers

P2

```text
With no secrets key, an instance admin opens a new Matrix route for a project, asks that form for a preview, and saves it.
```

```text
Each of the three answers reads the shipped sentence that Matrix is not set up, followed by the offer to set integrations up for this Foundry alone, and following it shows her the instance integrations page.
```

```text
An instance admin fixes a missing Matrix setup from the route form where she noticed it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin is sent to the setup from a Matrix route form\, its preview and its save

```text
the offer pointed at a page that merely answers rather than at the instance integrations page; the landing step pins which page it is. The save refuses in place rather than answering plainly, and the reader takes that re-rendered form as the page it is, so the row is decided by its offers and not by how the refusal is answered.
```

###### Obligations



##### Covers

P2

P5

```text
With the Slack app given to this Foundry by its operator and no secrets key, an instance admin opens a new Slack route for a project.
```

```text
She reads the shipped sentence that Slack cannot be connected until the secrets key is set, followed by the offer to set integrations up for this Foundry alone, which opens for her.
```

```text
An instance admin is sent to the key she is missing, from the form that told her it was missing.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin is sent to the setup from a form that says Slack has no secrets key

```text
the form also offered how to set up Slack here, where the app is already set up; the expected list is the whole offer, so an extra one fails it.
```

###### Obligations


##### Covers

P3

```text
With this Foundry holding a secrets key and no Slack app, a team lead who is this Foundry's instance admin but not a workspace admin opens a new Slack route for her team's project.
```

```text
She reads the shipped sentence that Slack is not set up, is offered setting up integrations for this Foundry alone and not the Slack guide, and following that offer shows her the instance integrations page.
```

```text
Someone who can only fix this at the instance level is offered exactly that, and never a workspace page she cannot open.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: An instance admin who is not a workspace admin is offered the setup alone

```text
the two facts, workspace admin and instance admin, were collapsed into one admin flag that happens to be true for her; the workspace-admin row and the lead row hold the other two combinations.
```

###### Obligations


##### Covers

P3

P5

```text
With this Foundry holding a secrets key and no Slack app, a team lead who is neither a workspace admin nor an instance admin opens a new Slack route for her team's project.
```

```text
She reads the same shipped sentence that Slack is not set up and is offered no link anywhere on that page.
```

```text
A lead is not sent to a setup or a guide she cannot open; the page tells her what is missing and stops there.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A lead who is neither admin is offered no way out of a form that says Slack is not set up

```text
the page offered her nothing because the observation never looked; the instance-admin rows read the same way and find two offers, which is what tells the empty case apart from a blind one.
```

###### Obligations



##### Covers

P5

P3

```text
With a saved Slack connection this Foundry can no longer read, an instance admin opens a new Slack route for a project.
```

```text
She reads the shipped sentence that Slack is unavailable because this Foundry cannot read its saved connection and an operator needs to check the secrets key, and she is offered no link at all.
```

```text
A reader of an unreadable connection is told what is wrong without being offered a setup that would not repair it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A connection this Foundry cannot read still names the key\, and offers no setup

```text
the sentence were kept while a setup offer appeared for some other state on the same page; this form shows one integration at a time, so the empty list is that sentence's whole offer.
```

###### Obligations


##### Covers

P5

P3

```text
With Slack set up on this Foundry and the workspace not connected to it, an instance admin opens a new Slack route for a project.
```

```text
She reads the shipped sentence that the workspace has no Slack connection yet, followed by the shipped offer to connect Slack in workspace integrations alone, which opens for her.
```

```text
Where the missing step is the workspace's own connection, the page still sends her there and not to the instance setup.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A workspace nobody connected still sends an instance admin to the workspace page\, and nowhere else

```text
the instance setup were offered after the connect offer for this state as well; the expected list is the whole offer, so the extra one fails it.
```

###### Obligations


##### Covers

P5

P3

```text
With this Foundry holding a secrets key and the workspace connected to no chat, an instance admin opens a project's integrations page.
```

```text
She reads the shipped sentence that the workspace has no chat connection yet, and the page's offers read exactly as they ship, connecting Slack or Matrix in workspace integrations and then making a new route, both of which open for her.
```

```text
The project page an instance admin reads is the page everyone else reads: the change adds nothing to its lines.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: A project page reads to an instance admin exactly as it ships

```text
a connected or lost connection line gained an offer; the shipped integrations-wayfinding scenarios read those two lines for the same viewer.
```

###### Obligations



##### Covers

P3

```text
Everyone who is not this Foundry's instance admin, a workspace admin, a lead, a member and a signed-out visitor, asks for the instance integrations page directly.
```

```text
Each of them gets the same not-found page any unknown address gets, so the page an offered setup leads to refuses exactly the viewers the offer is withheld from.
```

```text
Nobody reaches this Foundry's setup by typing its address, and nobody learns that it exists.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/instance\-integration\-setup\.feature \:\: Only an instance admin reaches the page or changes anything on it

```text
the guard were tightened further so that an instance admin is refused too; the rows that follow the offer and land on that page refuse that.
```

###### Obligations


##### Covers

P3

```text
Every combination of reason a kind has no connection, whether the workspace has one, and the two facts about the viewer, read at once rather than one page at a time.
```

```text
Every one of those combinations offers only links that viewer can open, with the offer to set this Foundry up last where it is offered at all.
```

```text
No reader of any of those states is offered a link that dead-ends, including the combinations no page of this Foundry can show today.
```

##### Disposition

NotTested

```text
The public pages reach only the combinations a viewer can actually be in: the workspace page is behind the workspace-admin guard, and a route form shows one kind at a time. The whole relation is decided where the words and offers are chosen, by the exhaustive table the delivery's own check runs over that choice, which is not a public port.
```

crafter\, in the no\-connection words table the design names as an extended target\, run by the declared foundry\-app library check

##### Covers

P3

```text
The whole set of parts this Foundry is built from is checked for a lower part that has come to depend on the part answering who is an instance admin.
```

```text
No such dependency exists: who is this Foundry's instance admin stays a question the web part answers, and the stored-data and shared-service parts never learn it.
```

```text
The per-viewer offer keeps one owner, so a later change cannot make a second, disagreeing answer about who may be sent to the setup.
```

##### Disposition

Tested

xtask\/src\/check\_arch\.rs \:\: the crate\-graph dependency\-direction guard\, run by cargo xtask check\-arch

```text
the fact were read in the web part and then passed downward as a plain value through a service call; the guard reads dependencies between parts, not arguments.
```

###### Obligations


## Expectations

### Rows

#### E1

##### Promises

P1

```text
Signed in as an instance admin, on the workspace integrations page I read the Slack W-2 text and immediately after it find the existing "How to set up Slack" link, and after that link the words "Set up integrations for this Foundry"; choosing the latter opens the instance admin integrations page. The order (text, then any existing setup link, then "Set up integrations for this Foundry") is part of this claim. Contract-grounded in P1.
```

#### E2

##### Promises

P1

```text
As the same instance admin I also find "Set up integrations for this Foundry" after the Matrix W-2 text and after the W-3 text. Ambiguity for the product owner: P1 names an existing "How to set up Slack" link only for Slack W-2, so whether Matrix W-2 or W-3 has its own existing setup link is not stated; where no such link exists, only "Set up integrations for this Foundry" follows the text and that absence is not a failure.
```

#### E3

##### Promises

P1

```text
The new links only add. From the same workspace integrations page an instance admin can still do what US-IW-02 AC 1 promised: the workspace integration action offered before is still offered and still completes, and the added links neither replace nor obstruct it. Contract-grounded in P1's "US-IW-02 AC 1 and KPI-IW-3 stay valid".
```

#### E4

##### Promises

P2

```text
On the project new-route form I am offered the same links for the same integration kind and the same state as on the workspace integrations page, worded identically: no kind gains or loses a link between the two pages, and no sentence is paraphrased.
```

#### E5

##### Promises

P2

```text
When I ask the new-route form for a preview, and when I create the route, the response I land on offers those same links with the same words for the same state. Ambiguity for the product owner: P2 names the preview and create posts but not what they render, so if a post answers with a redirect instead of the form, this claim can only be judged on the page that actually shows the form.
```

#### E6

##### Promises

P3

```text
As a workspace admin who is not an instance admin I am offered "How to set up Slack" and am never offered "Set up integrations for this Foundry" - not on the workspace integrations page and not on the new-route form.
```

#### E7

##### Promises

P3

```text
As a lead who is neither a workspace admin nor an instance admin I am offered no setup link at all on those pages: neither "How to set up Slack" nor "Set up integrations for this Foundry".
```

#### E8

##### Promises

P3

```text
Every link I am offered, I can open. Following the offered instance admin integrations link, or the offered Slack setup link, shows me that page rather than a permission denial, a sign-in prompt or an error page.
```

#### E9

##### Promises

P4

```text
On the Slack setup page, step 3 reads exactly "An instance admin enters the app's Client ID, Client Secret and Signing Secret under Instance admin › Integrations." and step 4 reads exactly "No restart is needed. A workspace admin can then choose Add to Slack on the workspace integrations page."
```

#### E10

##### Promises

P4

```text
The Slack setup page carries a footnote telling me that SLACK_CLIENT_ID, SLACK_CLIENT_SECRET and SLACK_SIGNING_SECRET take precedence, and no step anywhere on that page tells me to restart Foundry.
```

#### E11

##### Promises

P4

```text
The Slack app manifest the setup page gives me is the same as before this change, so a Slack app I already created from it stays correct. Limitation: judging "unchanged" needs the previously published manifest to compare against; this handoff does not supply it.
```

#### E12

##### Promises

P5

```text
On the workspace integrations page and the new-route form, the W-2, W-3, K-2 and K-19 wording reads exactly as it did before this change: a user who knew those sentences finds no word rewritten, only the new links around them. Limitation: this handoff supplies the identities but not the prior text of W-2, W-3, K-2 and K-19, so judging it needs that published prior wording.
```

#### E13

##### Promises

P6

```text
An operator reading RELEASING.md R-1 and R-2 is told to set up Slack and Matrix in the browser under Instance admin › Integrations, and can finish both integrations by following only those browser instructions.
```

#### E14

##### Promises

P6

```text
deploy/k8s/README.md also describes setting Slack and Matrix up in the browser under Instance admin › Integrations, and both documents present the environment settings as an optional operator alternative rather than a required step.
```

## Charter

None.

## Examination

None.

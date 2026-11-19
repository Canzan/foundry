# Acceptance brief: project\-key\-length

## Slices

### S1

##### Covers

P1

```text
An operator posts the project create form with the name Platform Operations and the key PLATFORMOP.
```

```text
The project is created, she lands on its board, the board header shows the key PLATFORMOP in full, and the project list she returns to shows PLATFORMOP.
```

```text
She gets a project whose ten-letter key is exactly the one she typed, shown whole wherever she reads it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A ten\-letter key creates the project and is shown in full

```text
A product that accepts exactly ten letters and refuses seven to nine, because this row exercises the ten-letter end alone; the two-letter row, the accepted-key table and the generated domain law over every length close that gap.
```

###### Obligations


##### Covers

P1

```text
An operator posts the same form with the two-letter key PL.
```

```text
The project is created and the key PL is shown in full on its board and in her project list.
```

```text
The longer limit takes none of the short keys away that she could already use.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A two\-letter key is still accepted and shown in full

```text
A product that accepts only the two ends of the range, since this row and the ten-letter row read the ends; the accepted-key table and the generated domain law cover the lengths between them.
```

###### Obligations


##### Covers

P1

```text
An operator offers the create form the keys AU, AUTH, AUTHWS and PLATFORMOP in turn.
```

```text
Each key is accepted and the create goes through.
```

```text
Every key of two to ten capital letters is one she may use, including the short ones she could use before.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A key of two to ten capital letters is accepted on the create form

```text
A form that accepts a key the stored rule would refuse, because this row reads the form's answer only; the executed parity of the domain rule against the real database closes that.
```

###### Obligations






##### Covers

P2

```text
An operator posts the create form with an eleven-letter key, a twelve-letter key, a one-letter key, an empty key, two lower-case keys, two keys carrying a digit, a key carrying a hyphen, a key carrying a space, a key carrying an underscore, a ten-letter key carrying an accented letter and a four-character key written in Japanese.
```

```text
Each attempt comes back refused on the form with one sentence stating that a project key must be 2 to 10 capital letters A to Z, and no project exists afterwards under the typed key or under a tidied-up version of it.
```

```text
Whatever she got wrong, she reads the whole rule in one sentence and her project list is untouched.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A key outside two to ten capital letters is refused with the whole rule and creates nothing

```text
A refusal sentence that states the rule while the stored rule stays narrower than it, because this row reads the operator's answer only; the executed parity against the real database closes that.
```

###### Obligations














##### Covers

P1

```text
In a project whose key is PLATFORMOP, the operator files her first issue.
```

```text
The issue is identified as PLATFORMOP-1, and the board lists that one issue under that key and no other.
```

```text
Her issue carries the whole project key she chose, with the number one after it.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: The first issue of a ten\-letter\-key project carries the whole key

```text
A product that keys the first issue correctly and shortens later ones, because this row stops at the first issue; the later-issue row and the side-by-side row cover the rest.
```

###### Obligations


##### Covers

P1

```text
In the same project, after nine issues have been filed, the operator files one more and then reopens the board and an earlier issue.
```

```text
The new issue is PLATFORMOP-10, the board lists PLATFORMOP-1 to PLATFORMOP-10 and nothing else, and the page of the seventh issue shows PLATFORMOP-7.
```

```text
Every issue she files or revisits keeps the whole key and its own number, including past the ninth.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A later issue of a ten\-letter\-key project keeps the whole key

```text
A product that renders issue keys correctly on the board and the issue page but shortens them in search results or in a notification, because this row reads the board and the issue page only.
```

###### Obligations


##### Covers

P3

```text
An operator opens a project that already carries a two-letter key, and another that carries a six-letter key, each with three issues already filed.
```

```text
Each board opens, shows its own key unchanged and lists exactly the three issue keys it already had, and an earlier issue still opens under its own key.
```

```text
Nothing she wrote down about an older project becomes wrong.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A project with a shorter key keeps its key and its issue keys

```text
An upgrade that re-minted the keys of projects written under the earlier rule, because these older projects are seeded after the stored rule is widened; the migration's own preservation check observes the upgrade itself.
```

###### Obligations




##### Covers

P3

```text
In the project that already carries the two-letter key and three issues, the operator files a new issue.
```

```text
The new issue is the fourth under that same unchanged key, and the board lists the first four issues and nothing else.
```

```text
She goes on working in her older project exactly as before.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A project with a shorter key still takes new issues under that key

```text
A product that keeps the key but renumbers from one again, which the exact list of the first four issues would catch only for this one project; the side-by-side row covers numbering across projects.
```

###### Obligations


##### Covers

P3

```text
A project carrying a two-letter key with nine issues and a project carrying the ten-letter key with nine issues stand side by side, and the operator files one more issue in the longer-key one.
```

```text
The longer-key board lists its own ten issues and no issue of the other project, and the shorter-key board still lists its own nine and none of the longer-key ones.
```

```text
Each project keeps its own issue keys; none of her keys points at the other project.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/us\-07\-project\-create\.feature \:\: A short\-key and a ten\-letter\-key project keep their own issue keys side by side

```text
Two projects whose keys share a prefix mixing up at a higher number, because this row pairs a two-letter and a ten-letter key that share no prefix.
```

###### Obligations


##### Covers

P1

```text
Keys of every length from nothing to fourteen characters are offered to the rule, made of capital letters, lower-case letters, digits, punctuation, a space and characters outside the Latin alphabet, together with the named keys AU, AUTHWS, PLATFORMOP, PLATFORMOPS, a single letter, nothing at all, auth, AUTH-X, AU1 and four characters written in Japanese.
```

```text
A key is accepted exactly when it is 2 to 10 characters and every one of them is a capital letter A to Z, an accepted key keeps the text that was typed, and every other key is refused.
```

```text
Every key an operator can type gets the answer the stated rule gives, with no key quietly accepted or quietly refused.
```

##### Disposition

Tested

crates\/foundry\-core\/tests\/project\_key\.rs\:\:a\_key\_is\_accepted\_exactly\_when\_it\_is\_two\_to\_ten\_capital\_letters

```text
A rule that counts bytes instead of characters for keys written outside the Latin alphabet, because every such key is refused either way; only the refusal arm would differ, and the operator reads the same sentence for both.
```

###### Obligations












##### Covers

P3

```text
Every key of two to six capital letters, the keys AU and AUTHWS among them, is offered to the rule again.
```

```text
Each one is still accepted.
```

```text
No key an operator could already use stops working.
```

##### Disposition

Tested

crates\/foundry\-core\/tests\/project\_key\.rs\:\:every\_key\_the\_earlier\_rule\_accepted\_is\_still\_accepted

```text
A rule that widened the upper end but moved the lower end, which the two-letter end of this range is what catches.
```

###### Obligations




##### Covers

P1

```text
Keys of two to ten capital letters are paired with issue numbers from one to a thousand, PLATFORMOP with number one among them.
```

```text
Each issue key reads as the project key verbatim, a hyphen, then the number, with the expected text assembled from the key and the number rather than read back from the rendering.
```

```text
Her issue keys carry the whole project key, whatever its length and however high the number.
```

##### Disposition

Tested

crates\/foundry\-core\/tests\/project\_key\.rs\:\:an\_issue\_key\_reads\_as\_the\_project\_key\_then\_the\_number

```text
A rendering that is right in the domain and shortened by a template, which the board and issue-page rows observe instead.
```

###### Obligations



##### Covers

P1

P2

```text
The same keys are offered twice over, each as a real write into a real database and as a question to the rule the application enforces: AU, PLATFORMOP, PLATFORMOPS, auth, AU1, nothing at all, and keys drawn at every length up to fourteen characters.
```

```text
For every key the two answers agree: the database stores exactly the keys the application accepts, and refuses exactly the keys it refuses.
```

```text
A create the product accepts is never turned into a server error by the store, and no key shape slips into the store behind the product's back.
```

##### Disposition

Tested

crates\/foundry\-store\/tests\/project\_key\_db\_rule\.rs\:\:the\_stored\_rule\_admits\_exactly\_the\_keys\_the\_application\_accepts

```text
A write that reaches the store through a path that does not pass the rule at all, since this row compares verdicts on the same text rather than auditing every writer.
```

###### Obligations









##### Covers

P3

```text
A project keyed with two letters and a project keyed with six letters, the two ends of what the earlier rule allowed, are written while the database still carries the earlier rule; then the widening is applied.
```

```text
Both rows read exactly as before, both keys are still accepted by the rule, and the database now also admits a ten-letter key.
```

```text
An older project is left exactly as it was, and the longer keys become usable in the same move.
```

##### Disposition

Tested

crates\/foundry\-store\/tests\/project\_key\_db\_rule\.rs\:\:widening\_the\_stored\_rule\_leaves\_every\_earlier\_project\_untouched

```text
A widening that leaves the rows alone but takes a long lock while applying, which a deploy observes rather than this row.
```

###### Obligations




##### Covers

P2

```text
A throwaway copy of the codebase is given a second copy of the very sentence the product gives an operator whose key is refused, once in the web code, once in the services code and once in the store code; the architecture gate is then run over each copy, and over an untouched copy.
```

```text
Each planted copy is refused by the gate, which names the file holding the second sentence, and the untouched copy is accepted.
```

```text
The rule the operator reads cannot drift from the rule the product enforces, because a second copy of that sentence cannot be added unnoticed.
```

##### Disposition

Tested

crates\/foundry\-acceptance\/tests\/features\/project\-key\-length\-one\-source\.feature \:\: A second copy of the refusal sentence is refused and named

```text
A gate that bites a second copy only in the one file each row plants, or only in the three crates these rows reach, since the law covers every file those crates hold now or gain later and these rows plant one file per crate.
```

###### Obligations






## Expectations

### Rows

#### E1

##### Promises

P1

```text
An operator who fills the project form with a 10-letter capital key such as PLATFORMOP gets the project created, and the project is listed and opened afterwards showing exactly that key, with no truncation and no change of letters.
```

```text
A check of the promise can use one single key length and one single form path, so a product that accepts only exactly 10 letters, or that stores the key correctly but shows a shortened form elsewhere, still looks correct to the user.
```

None.

None.

None.

#### E2

##### Promises

P1

```text
A 2-letter capital key such as PL is still accepted on the same form and creates a project with that key, so the longer limit does not remove the short keys an operator could already use.
```

```text
The promise names the lower end only inside a range, so a check can exercise the long key alone and never show that 2-letter keys remain accepted.
```

None.

None.

None.

#### E3

##### Promises

P1

```text
The first issue an operator files in a project with a 10-letter key is identified to the operator as PLATFORMOP-1, and each later issue in the same project keeps the same full key as its prefix with the next number.
```

```text
A check can observe one issue only, and only at the moment of creation, so a wrong or shortened prefix on later issues, in lists, in search results, or after a reload, stays unseen.
```

None.

None.

None.

#### E4

##### Promises

P2

```text
An 11-letter key is refused on the project form, the operator reads a message that names the 2 to 10 capital letter rule, and no project with that key exists afterwards in the project list.
```

```text
A check can assert that some error text appears and stop there, so a message that omits the limits, or a refusal that still leaves a created project behind, can pass.
```

None.

None.

None.

#### E5

##### Promises

P2

```text
A key of 1 letter, and an empty key, are refused on the same form with the same 2 to 10 capital letter rule message, and no project is created.
```

```text
The promise groups too short with too long, so a check can cover the long case alone; an empty field may also be answered by a separate generic required-field message that the check accepts as the rule message.
```

None.

None.

None.

#### E6

##### Promises

P2

```text
A key that contains any character other than A-Z, including a lower-case letter, a digit, a space, a hyphen, an underscore or an accented letter, is refused with the same rule message and no project is created; the form does not quietly correct the key and create the project instead.
```

```text
A check can use one invalid character only, so other classes stay unobserved, and a form that silently upper-cases or strips characters before it validates looks identical to a correct refusal unless the resulting project list is also observed.
```

None.

None.

None.

#### E7

##### Promises

P3

```text
A project that an operator created before this change still shows its original key, and every issue it already had still shows the same issue key as before, so no existing reference an operator wrote down becomes wrong.
```

```text
A check can create its subject after the change and call it existing, which cannot show what happens to data written under the earlier rule; nothing observable from outside states which upgrade path the installation took.
```

None.

yes

```text
A project with a 2 to 6 letter key and at least one filed issue that exists before the change is applied, then observed after it.
```

#### E8

##### Promises

P3

```text
In that pre-change project an operator can still do the ordinary work: open the project, file a new issue that gets the next number under the unchanged key, and open an issue that already existed.
```

```text
Keeps working has no stated boundary, so a check can sample a few actions only; a sample cannot show that every pre-change project and every operation in a real installation is unaffected.
```

None.

yes

```text
The same pre-change project and its existing issues, observed after the change.
```

#### E9

##### Promises

P3

```text
A pre-change project and a new 10-letter-key project exist side by side, and the issue keys of the two do not mix or collide: each issue keeps its own project prefix.
```

```text
This is only observable when both kinds of project are present at once; a check that uses one project at a time cannot show the coexistence, and a short observation cannot show collisions that appear only at higher issue numbers.
```

no

None.

```text
The pre-change project kept, plus a project created after the change with a 10-letter key.
```

## Charter

None.

## Examination

None.

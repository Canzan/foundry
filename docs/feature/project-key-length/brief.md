# Product brief: project\-key\-length

## Request

```text
change the max length of the project key to <=10
```

## Work

S

```text
An operator can give a project a key of up to 10 capital letters, so issue keys can carry a longer, clearer prefix
```

### Rework

None.

## Promises

### P1

Outcome

```text
An operator creates a project whose key is 2 to 10 capital letters A-Z, such as PLATFORMOP, and every issue of that project carries that key as its prefix, such as PLATFORMOP-1.
```

### P2

Outcome

```text
A key shorter than 2 letters, longer than 10 letters, or with any character other than A-Z is refused on the project form with a message that states the 2 to 10 capital letter rule, and no project is created.
```

### P3

Outcome

```text
Every project that exists before the change keeps its key and its issue keys unchanged, and keeps working.
```

## Scope

```text
The project key length rule on project creation: 2 to 10 capital letters A-Z, in the domain, the database and the form message.
```

```text
Renaming the key of an existing project; lower-case or digit keys; any change to the 2-letter minimum; a Teams item in the left nav.
```

## Decisions

None.

## Journey

None.

## Slices

### S1

```text
An operator creates a project with a 10-letter key and files an issue that carries it; an 11-letter key is refused with the 2 to 10 rule; an existing 2-6 letter project still works.
```

#### Promises

P1

P2

P3

#### Depends on

None.

#### Slice work

None.

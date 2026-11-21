# BARE\-PAGES\-001 Both shortcut arms vary on the declared destination
Status: Accepted

## Context

```text
ADR-002 keeps one URL for the shortcut list and decides its body from the Sec-Fetch-Dest request header. It settled WHICH body each client receives and said nothing about caches. A single URL whose body depends on a request header is exactly the shape an intermediate or private cache gets wrong: it stores the first body it sees under the URL and answers every later request from it, so the overlay fetch can receive a framed document and a navigation can receive a bare fragment, with no code path at fault and nothing failing in the application's own tests.
```

## Decision

```text
Both arms declare the dependency in their response: GET /keyboard-help answers Vary: Sec-Fetch-Dest on the page arm AND on the fragment arm, set in one place beside the arm decision. Everything ADR-002 decided stands: one URL, one row source, the document destination as the only page arm, the fragment as the safe default. The oracle owes one read of the header on both arms.
```

## Alternatives

```text
Cache-Control: no-store on both arms (rejected: it forbids the browser's own back-forward reuse of the help page and buys nothing Vary does not already give, since the body is public, small and identical for every client of one arm). Vary on the page arm alone (rejected: the fragment is the body a cache is most likely to hold, because every overlay press fetches it, so the unmarked arm is the dangerous one). A second URL for the page, so no header decides a body (rejected by ADR-002 for reasons that stand: a shipped read asserts the user menu links to this exact URL, and two URLs make two row sources to keep equal).
```

## Consequences

```text
A cache keys its entry on the declared destination, so the two arms cannot cross. A client that declares no destination shares the fragment entry with the overlay fetch, which is correct: they receive the same body. Any later route that decides a body from a request header owes the same declaration; today this is the only such route in the application. The header is part of the response contract, so the oracle reads it on both arms and a response missing it is a defect even with the right body.
```

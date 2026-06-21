# Memory-Format Token/Space Benchmark — RESULTS

- Facts: **30** canonical memory facts (prefs, constraints, decisions+why, code-locations, relations, facts).
- Tokenizer: **tiktoken/cl100k_base**. Caveat: cl100k_base ≠ Anthropic tokenizer — numbers are RELATIVE (format-vs-format), not absolute Claude counts.
- Lossy note: `kg_triples` DROPS the `why` rationale and id/type metadata — densest but lossy.

## Ranked (fewest tokens first)

| format | tokens | tokens/fact | bytes | % token savings vs json_pretty |
|---|---|---|---|---|
| kg_triples | 489 | 16.3 | 1959 | 71% |
| md_cards | 674 | 22.5 | 2702 | 61% |
| json_min | 1107 | 36.9 | 4295 | 35% |
| yaml | 1252 | 41.7 | 4072 | 27% |
| json_pretty | 1707 | 56.9 | 5442 | 0% |

**Densest:** `kg_triples`. **Best lossless (keeps rationale):** `md_cards`.

## Readability samples (first 3 facts)

### json_pretty
```
[
  {
    "id": "f01",
    "type": "pref",
    "subject": "user",
    "rel": "prefers",
    "object": "no Co-Authored-By lines in git commits"
  },
  {
    "id": "f02",
    "type": "pref",
    "subject": "user",
    "rel": "prefers",
    "object": "caveman-terse responses to save tokens"
  },
  {
    "id": "f03",
    "type": "pref",
    "subject": "user",
    "rel": "uses",
    "object": "Arch Linux with zsh shell"
  }
]
```

### json_min
```
[{"id":"f01","type":"pref","subject":"user","rel":"prefers","object":"no Co-Authored-By lines in git commits"},{"id":"f02","type":"pref","subject":"user","rel":"prefers","object":"caveman-terse responses to save tokens"},{"id":"f03","type":"pref","subject":"user","rel":"uses","object":"Arch Linux with zsh shell"}]
```

### yaml
```
- id: f01
  type: pref
  subject: user
  rel: prefers
  object: no Co-Authored-By lines in git commits
- id: f02
  type: pref
  subject: user
  rel: prefers
  object: caveman-terse responses to save tokens
- id: f03
  type: pref
  subject: user
  rel: uses
  object: Arch Linux with zsh shell
```

### md_cards
```
- **user** prefers no Co-Authored-By lines in git commits
- **user** prefers caveman-terse responses to save tokens
- **user** uses Arch Linux with zsh shell
```

### kg_triples
```
user | prefers | no Co-Authored-By lines in git commits
user | prefers | caveman-terse responses to save tokens
user | uses | Arch Linux with zsh shell
```

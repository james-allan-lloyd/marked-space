---
metadata:
  author: Olivia
  status: on track
emoji: heart_eyes
imports:
  - adr.md
---

# Metadata Example

This example shows how to define a macro that uses metadata to generate a
table.

{{ adr::header_table() }}

## Referencing metadata

You can reference metadata in the following way:

```markdown
---
metadata:
  author: James
---

# Title

{{ '{{ metadata(path="author") }}'}}
```

As of `1.0.4`, metadata can also be referenced via the `fm` variable:

```markdown
---
metadata:
  author: James
---

# Title

{{ '{{ fm.metadata.author }}'}}
```

This allows some extra work to be done in the templating engine. For instance,
you can print out all of the metadata like this:


| key | value |
| --- | --- |
{% for key, value in fm.metadata -%}
| {{ key}} | {{value}} |
{% endfor %}

---
metadata:
  owner: Olivia
  status: Approved
labels:
  - page-properties
---

# Page with page properties

So given that we've specified the following in our front matter:

```markdown
---
metadata:
  owner: Olivia
  status: Approved
labels:
  - page-properties
---
```

When we add the following

```text
{{ '{{ builtins::properties() }}' }}
```

We should get a nice table:

{{ builtins::properties(metadata=['owner', 'status']) }}

> [!NOTE]
> Unfortunately, we can't supply any extra formatting to the fields at the
> moment. If you want to use other formatting (such as the status macro), then
> you will need to generate the table yourself.

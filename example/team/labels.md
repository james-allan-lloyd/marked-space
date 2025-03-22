---
labels:
  - howto
  - intro
---

# Labels

Labels are supported for your Confluence pages via "front matter" (this is
currently the only supported use for front matter). Front matter is parsed as
YAML surrounded by triple dashes like so:

```markdown
---
<!-- Single line: labels: ['label-A', 'label-B'] -->
labels:
- 'label-A'
- 'label-B'
---

# The Title of the File

Contents
```

## Displaying Pages by Label

Once you've labelled some of your pages you can then use the label list builtin
macro function to display a list of pages with a particular macro:

```markdown
{{ '{{labellist(labels=["foo", "bar"])}}' }}
```

And you should see the following:

{{labellist(labels=["foo", "bar"])}}

Currently the label list will include pages with _any_ of the given labels. The
actual confluence macro is quite flexible as it uses cql underneath; future
versions of `marked-space` may include more support for generating it. You can
also write your own macro to do so (see [](macros.md)!).

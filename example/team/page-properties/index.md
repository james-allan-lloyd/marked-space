# Page Properties

To use the properties_report, you need to label the pages you want to appear in
the table with some label. Then you can display them in a table with the
following code:

```markdown
{{ '{{ builtins::properties_report(label="page-properties") }}' }}
```

Which should produce something like this:

{{ builtins::properties_report(label="page-properties") }}

You will have to specify some properties on that page... see [this
example](./page.md) for how to use the page metadata.

{% import "_tera/example-macros.md" as macros %}
# Macros

There is basic macro support available using [Tera](https://keats.github.io/tera/docs), with the following "builtin" functions:

- `{{ '{{toc()}}' }}` inserts the confluence Table of Contents macro
- `{{ '{{children()}}' }}` inserts the confluence Children macro
- `{{ '{{filename}}' }}` inserts the current filename (which for this file is `{{filename}}`)

You can also write your own macros and place them in files in the `_tera` directory under your space directory (otherwise they'll be interpreted as confluence pages). As the first line of the file, you should import them (as we did with this file: there's a `{{ "{% import '_tera/example-macros.md' as macros %}" }}` at the top of this file). You can then call the macros defined within like this:

```markdown
{{ "{{macros::example_macro(name='Your Name')}}" }}
```

Which should give:

```markdown
{{macros::example_macro(name='Your Name')}}
```

Note that you _must_ use the keyword arguments or Tera will complain "expected an identifier".

You can also [include templates](https://keats.github.io/tera/docs/#include) from the `_tera` directory, and even [extend them](https://keats.github.io/tera/docs/#inheritance). See [Subpage Example](subpages/index.md) for a further example.

## HTML Rendering

In order to allow macros to generate Confluence macro references, the Comrak's sanitisation of HTML in Markdown had to be disabled. However, certain tags will still be omitted. For instance:

{{ '<script/>' }} should be santised to just return the html as text: <script>document.getElementById("demo").innerHTML = "Hello JavaScript!";</script>. In contrast, writing {{ '<em>Bold</em>' | escape }} should result in <em>Bold</em>.
{% import "_tera/example-macros.md" as macros %}

# Macros

There is basic macro support available using
[Tera](https://keats.github.io/tera/docs), with the following "builtin"
functions:

- `{{ '{{toc()}}' }}` inserts the confluence Table of Contents macro
- `{{ '{{children()}}' }}` inserts the confluence Children macro
- `{{ '{{filename}}' }}` inserts the current filename (which for this file is `{{filename}}`)

You can also write your own macros and place them in files in the `_tera`
directory under your space directory (otherwise they'll be interpreted as
confluence pages). As the first line of the file, you should import them (as we
did with this file: there's a `{{ "{% import '_tera/example-macros.md' as
macros %}" }}` at the top of this file). You can then call the macros defined
within like this:

```markdown
{{ "{{macros::example_macro(name='Your Name')}}" }}
```

Which should give:

```markdown
{{macros::example_macro(name='Your Name')}}
```

Note that you _must_ use the keyword arguments or Tera will complain "expected an identifier".

You can also [include templates](https://keats.github.io/tera/docs/#include)
from the `_tera` directory, and even [extend
them](https://keats.github.io/tera/docs/#inheritance).

## HTML Rendering

In order to allow macros to generate Confluence macro references, the Comrak's
sanitisation of HTML in Markdown had to be disabled. However, certain tags will
still be omitted. For instance:

{{ '<script/>' }} should be santised to just return the html as text:

<script>document.getElementById("demo").innerHTML = "Hello
  JavaScript!";</script>.

In contrast, writing {{ '<em>Bold</em>' | escape }} should result in
<em>Bold</em>.

## Exposing Confluence Macros

It is possible to expose most of the macros in Confluence. marked-space does this for a couple of common marcos, but given that you may have many macro plugins installed in your instance, we don't supply them. How might you do this yourself, especially given the unknown uuid that identifies the macro?

The easiest way is to create a test page in Confluence with your macro, setting
some of the configuration you'd like to expose in your own marked-space/tera
macro. Save the changes and then go to '...' -> 'Advanced Details' -> 'View
Storage Format'. This should give you some example to copy and paste into your
own macro files.

Note that Confluence can be quite sensitive to whitespace within the macro. For
this reason, if you have to use control structures, prefer to use the "consume
whitespace" marker - ie, a `-` next to the opening and closing tera tag, ala
'{{ "{%- if something -%}something{%- endif -%}" }}'.

Using the 'View Storage Format' is also a good way to debug when the template
generation doesn't seem to be working.


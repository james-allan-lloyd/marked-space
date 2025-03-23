# Mentions

Mentions are a way to reference other users in Confluence. You can do the same
in your markdown like this:

{{ mention(public_name="James Lloyd") }}

Public name here is the public name on the server. Usually this will match
watch would be shown when you normally use `@` while writing in Confluence
directly.

## Missing Users

If no matching user can be found, then the template will insert @unknown_user
and a warning will be printed.

{{ mention(public_name="Some Guy") }}

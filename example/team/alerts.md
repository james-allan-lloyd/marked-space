# Alerts and Expandable Sections

Notes:

> [!NOTE]
> Useful information that users should know, even when skimming content.

Tips:

> [!TIP]
> Helpful advice for doing things better or more easily.

Important:

> [!IMPORTANT]
> Key information users need to know to achieve their goal.

Warning:

> [!WARNING]
> Urgent info that needs immediate user attention to avoid problems.

Caution:

> [!CAUTION]
> Advises about risks or negative outcomes of certain actions.

By default, `marked-space` will use the alert type as the title for the panel;
you can also supply your own:

> [!CAUTION] Danger, Will Robinson
> As the famous line goes...

You can also add the `[expand]` tag to make the block quote into a expandable
block. In this case the type of alert is ignored.

> [!NOTE] [expand] Some title
> My Body
>
> ```yaml
> folders: true
> ```
>
> Something else after the code.

This should have no title:

> [!NOTE] [expand]
> An expand without a title
>
> ```yaml
> folders: true
> ```
>
> Something else after the code.

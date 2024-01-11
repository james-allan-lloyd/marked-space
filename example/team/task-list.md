# Task Lists Example

Task lists are rendered in confluence storage format like this:

```xml
<ac:task-list>
    <ac:task>
        <ac:task-id>1</ac:task-id>
        <ac:task-status>incomplete</ac:task-status>
        <ac:task-body><span class="placeholder-inline-tasks">Top level task 1</span></ac:task-body>
    </ac:task>
    <ac:task-list>
        <ac:task>
            <ac:task-id>2</ac:task-id>
            <ac:task-status>complete</ac:task-status>
            <ac:task-body><span class="placeholder-inline-tasks">Sub task 1</span></ac:task-body>
        </ac:task>
    </ac:task-list>
    <ac:task>
        <ac:task-id>3</ac:task-id>
        <ac:task-status>incomplete</ac:task-status>
        <ac:task-body><span class="placeholder-inline-tasks">Top level task 2</span></ac:task-body>
    </ac:task>
</ac:task-list>
```

- [ ] This is a task list in github format
- [x] And another subtask, but it's done
  - [ ] And another subtask, but it's nested
- [ ] And back out to the top level.

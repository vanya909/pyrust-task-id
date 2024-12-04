# PyRust Task ID
![PyRust Task ID](https://img.shields.io/pypi/v/pyrust-task-id?label=pyrust-task-id)
![MIT](https://img.shields.io/badge/license-MIT-blue)
![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/vanya909/pyrust-task-id/.github%2Fworkflows%2FCI.yml)


A written in Rust tool for Python programs that automatically pull task id from branch name to commit message.

## Example
Let's imagine you have branch `project_name/TASK-111-implement-feature` and you need
to provide the task `TASK-111` into commit message.<br>Then you can just use this tool.

First, just add hook into `.pre-commit-config.yaml`:
```yaml
-   repo: https://github.com/vanya909/pyrust-task-id-pre-commit
    rev: 0.1.2
    hooks:
    -   id: pyrust-task-id
        stages: [commit-msg]
        args:
        -   "project_name/(?P<task_template>TASK-[0-9]{3})-.*"
        -   "{subject}\\n\\n{body}\\n\\nTask ID: {task_id}"
```

Then run
```bash
pre-commit install --hook-type "commit-msg"
```

Then commit
```bash
git commit -m"My cool feature" -m"Can't wait to see this feature in prod."
```

And then you'll see following commit message:
```
My cool feature

Can't wait to see this feature in prod.

Task ID: TASK-111
```

This project uses [standalone repo](https://github.com/vanya909/pyrust-task-id-pre-commit) for pre-commit hook because it requires pre-build python wheels from PyPI

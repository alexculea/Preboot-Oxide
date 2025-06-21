### Bugs or issues
Open an issue in the Github repository.

### Development notes

### Debugging
Visual Sudio Code is the recommended IDE.
Debugging is best done when the program runs as privileged and this can be achieved using the `lldb-server` (which needs to be installed separately). Once present,
`./vscode/launch.json` has `Remote LLDB as root` to match the launch configuration.

```BASH
# run the server before launching the debugger from VS Code
sudo su
lldb-server platform --server --listen 127.0.0.1:12345
```

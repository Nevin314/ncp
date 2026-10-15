# A Competitive Programming tool built in Rust

usage: ncp {command}

#### current commands:
- new (creates a new file based off a template you set)
- set-template (sets the default template file)
- run-sample (runs executable against sample test case, provided contest number and problem letter)
- start (imports a problem and starts its timer)
- stop (stops the timer and records the result)
- status (shows the active timer)
- list (shows imported problems)
- help

### Practice tracking

```sh
# Import the problem and start its timer in one command.
ncp start https://codeforces.com/problemset/problem/4/A \
  --name "Watermelon" --difficulty 800 --tags math,implementation

ncp status
ncp stop --rating 5 --outcome solved --notes "Found the key observation immediately"
ncp list
```

Solve ratings range from 1 (poor solve) to 5 (perfect solve). By default, the
database is stored at the operating system's local data path under
`ncp/tracking.db`. Set `NCP_DB_PATH` to make the CLI and a dashboard share a
specific database file.


#### Goals:
- make gen
    - automatically create all the problem files
    - download all simple test cases
- figure out/add the time it took for code to run?
- maybe view standings
- submit
- see if i can change to using requests (currently spawning curl command)

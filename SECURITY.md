# Security policy

## Supported versions

Only the latest release receives security fixes.

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Report them privately through GitHub instead:
[Report a vulnerability](https://github.com/SalomonAvila/memtop/security/advisories/new)
(the **Security** tab → **Report a vulnerability**).

Include the memtop version (`memtop -V`), your distribution and how to reproduce the
problem. You will get an answer as soon as possible; once a fix is released, the report can
be published with credit to you if you wish.

## Scope

memtop only reads local system information and never uses the network. The only action that
changes the system is sending a signal to a process, after asking for confirmation. Reports
about anything that breaks these guarantees are especially welcome.

# Companion launch configuration

The parser is a separate external application. This repository only launches it
and sends best-effort combat events to local UDP port 28015. It does not install
parser dependencies or modify the parser project.

## Configure once

Set EQOA_PARSER_DIR to your parser source or installation directory. For example,
in PowerShell before starting the launcher:

    $env:EQOA_PARSER_DIR = 'D:/Games/EQOA-Parser'
    ./Start-EQOA-With-Parser.bat

To retain the setting across new shells, set it in Windows Environment Variables.
Relative directories resolve from the client installation directory. Existing
terminals and launchers must be restarted to inherit a changed user setting.
The Python executable must be on PATH, or mod.json can name a bundled executable.
Install the separate parser's documented dependencies in that interpreter.

## Manifest contract

Native `mods` is now resolved beside the running client executable even when the
folder does not exist yet. The client no longer switches to the shell's working
directory merely because that executable-relative folder is missing. Open Mods
Folder creates it and the default parser manifest there. If the OS cannot locate
the executable, the client diagnoses that failure and uses a separate temporary
`eqoa-mods-unavailable` folder rather than selecting working-directory assets.
`EQOA_FORGE_MODS` is an explicit addon-runtime test override; it does not relocate
the companion parser's manifest. Existing parser command/working-directory rules
below remain intact.

mods/eqoa_parser/mod.json supplies command and args. The default command is
python with args -m eqoa_parser --client; that invokes the parser's __main__.py.
A bare command is found beside the manifest first, then on PATH. An explicit
relative command is resolved beside mod.json. The working directory is
EQOA_PARSER_DIR. Commands are launched directly without a shell. Native Windows
launch hides the console while allowing the parser GUI to open.

The desktop launcher, Start-EQOA-With-Parser.bat and native Mods & Addons action
follow the same contract. Missing configuration or executables produce an
explanation; native failures are recorded in the client log. The Running indicator
tracks only a child launched by that client; a separately started parser is not
automatically recognized, and the indicator is not proof of parser health.

## Browser and data limits

The browser does not spawn processes or open UDP sockets. Native combat messages
retain ts, actor, target, amount, ability and kind. Names are JSON escaped; heals
use positive amounts and kind=heal. This cleanup does not add group rosters,
remote spawn visibility or parser packaging. Those remain separate design work.
See parser-integration-design.md and MODDING-AND-PARSER-ROADMAP.md.

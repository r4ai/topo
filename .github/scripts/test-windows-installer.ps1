$ErrorActionPreference = 'Stop'
$installer = (Get-ChildItem dist/*-setup.exe | Select-Object -ExpandProperty FullName -Unique)
if (@($installer).Count -ne 1) { throw 'Expected one Windows installer' }
$installDir = Join-Path $env:RUNNER_TEMP 'topo-installed'
$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\topo.lnk'
$configDir = Join-Path $env:RUNNER_TEMP 'topo-install-user-config'
New-Item -ItemType Directory -Force $configDir | Out-Null
$marker = Join-Path $configDir 'preserved.txt'
Set-Content $marker 'Keep user settings'
$env:TOPO_CONFIG_DIR = $configDir
$expected = (Get-FileHash "target/$env:TARGET/release/topo-gui.exe").Hash
# Exercise installation and an in-place upgrade using the real published setup.
for ($attempt = 1; $attempt -le 2; $attempt++) {
    $log = Join-Path $env:RUNNER_TEMP "topo-install-$attempt.log"
    $process = Start-Process $installer -ArgumentList "/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR=`"$installDir`" /LOG=`"$log`"" -Wait -PassThru
    if ($process.ExitCode -ne 0) { Get-Content $log; throw "Installation failed: $($process.ExitCode)" }
    $binary = Join-Path $installDir 'topo-gui.exe'
    if ((Get-FileHash $binary).Hash -ne $expected) { throw 'Installed binary differs from the release build' }
    if (!(Test-Path $shortcut)) { throw 'Start menu shortcut missing' }
    $help = Start-Process $binary -ArgumentList '--help' -Wait -PassThru
    if ($help.ExitCode -ne 0) { throw 'Installed GUI executable failed to start' }
}
$uninstaller = Join-Path $installDir 'unins000.exe'
$process = Start-Process $uninstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -Wait -PassThru
if ($process.ExitCode -ne 0) { throw "Uninstall failed: $($process.ExitCode)" }
if (Test-Path (Join-Path $installDir 'topo-gui.exe')) { throw 'Uninstall left the GUI binary behind' }
if (Test-Path $shortcut) { throw 'Uninstall left the Start menu shortcut behind' }
if ((Get-Content $marker) -ne 'Keep user settings') { throw 'User settings were modified' }

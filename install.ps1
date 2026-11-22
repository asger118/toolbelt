# Installs (or updates) toolbelt's `tb` on Windows.
#   irm https://raw.githubusercontent.com/asger118/toolbelt/main/install.ps1 | iex
$ErrorActionPreference = 'Stop'
$repo = 'asger118/toolbelt'
$dir = Join-Path $env:LOCALAPPDATA 'toolbelt\bin'
$zip = Join-Path $env:TEMP 'tb-windows-x64.zip'

New-Item -ItemType Directory -Force -Path $dir | Out-Null
Invoke-WebRequest "https://github.com/$repo/releases/latest/download/tb-windows-x64.zip" -OutFile $zip
Expand-Archive -Path $zip -DestinationPath $dir -Force
Remove-Item $zip

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $dir) {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$dir", 'User')
    $env:Path += ";$dir"
    Write-Host "Added $dir to your PATH."
}
Write-Host "Installed $(& (Join-Path $dir 'tb.exe') --version)."
Write-Host 'Enable Ctrl+S by adding this line to your $PROFILE:'
Write-Host '  tb init powershell | Out-String | Invoke-Expression'

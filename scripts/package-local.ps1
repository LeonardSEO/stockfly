$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
python scripts/package-local.py @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

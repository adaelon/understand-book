param([int]$Port = 8895)
$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$root = Join-Path $repository '.understand-book/erlkoenig'
$publication = (Get-Content -LiteralPath (Join-Path $root 'local-publication.json') -Raw | ConvertFrom-Json).reference
$directory = Join-Path $root "local-library/library/published/$($publication.book_id)/$($publication.publication_id)"
if (-not (Test-Path -LiteralPath (Join-Path $directory 'publication.json'))) { throw 'The local music publication is missing' }
$executable = Join-Path $root 'bin/erlkoenig-server.exe'
$env:UNDERSTAND_BOOK_ADDR = "127.0.0.1:$Port"
$env:UNDERSTAND_BOOK_MEMORY_DIR = Join-Path $root 'reader-memory'
$env:UNDERSTAND_BOOK_WEB_DIST = Join-Path $root 'web-dist'
$sessionPath = Join-Path $env:UNDERSTAND_BOOK_MEMORY_DIR 'session.json'
if (Test-Path -LiteralPath $sessionPath) {
    $session = Get-Content -LiteralPath $sessionPath -Raw | ConvertFrom-Json
    $session.current_book_dir = $directory
    $session | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath $sessionPath -Encoding utf8NoBOM
}
New-Item -ItemType Directory -Path (Join-Path $root 'verification') -Force | Out-Null
$process = Start-Process -FilePath $executable -ArgumentList @('--reader-only', ('"' + $directory + '"')) -WorkingDirectory $repository -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'verification/server.log') -RedirectStandardError (Join-Path $root 'verification/server.stderr.log')
$reader = [pscustomobject]@{ process_id = $process.Id; url = "http://127.0.0.1:$Port/"; publication = $publication }
$reader | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'local-reader.json') -Encoding utf8
$reader | ConvertTo-Json -Depth 4
